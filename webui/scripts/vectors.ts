/**
 * Generates docs/test-vectors.json with node:crypto (an implementation independent of
 * src/lib/crypto.ts), then checks that crypto.ts agrees. Clients on other platforms
 * (Monica for Android) must pass the same vectors.
 *
 *   node scripts/vectors.ts            # verify only
 *   node scripts/vectors.ts --write    # regenerate the file
 */
import assert from "node:assert/strict";
import nodeCrypto from "node:crypto";
import fs from "node:fs";
import * as mc from "../src/lib/crypto.ts";
import { parseTotp, totpCode } from "../src/lib/totp.ts";

const out = new URL("../../docs/test-vectors.json", import.meta.url);
const hex = (b: Uint8Array) => Buffer.from(b).toString("hex");
const fromHex = (h: string) => new Uint8Array(Buffer.from(h, "hex"));
const b64 = (b: Uint8Array) => Buffer.from(b).toString("base64");

function gcm(key: Uint8Array, iv: Uint8Array, plain: Uint8Array, aad?: Uint8Array) {
  const c = nodeCrypto.createCipheriv("aes-256-gcm", key, iv);
  if (aad) c.setAAD(aad);
  return Buffer.concat([c.update(plain), c.final(), c.getAuthTag()]);
}
const enc = (key: Uint8Array, iv: Uint8Array, plain: Uint8Array) => `1.${b64(iv)}.${b64(gcm(key, iv, plain))}`;
const hkdf = (ikm: Uint8Array, info: string) => new Uint8Array(nodeCrypto.hkdfSync("sha256", ikm, new Uint8Array(0), info, 32));

function generate() {
  const email = "Test.User@Example.com ";
  const password = "Monica Cloud 测试 🔐";
  const iterations = 100_000;
  const pw = Buffer.from(password.normalize("NFC"), "utf8");
  const masterKey = nodeCrypto.pbkdf2Sync(pw, Buffer.from(email.trim().toLowerCase()), iterations, 32, "sha256");
  const authHash = b64(nodeCrypto.pbkdf2Sync(masterKey, pw, 1, 32, "sha256"));
  const wrapKey = hkdf(masterKey, "monica-cloud/v1/wrap");
  const userKey = fromHex("00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff");
  const itemKey = fromHex("f0e1d2c3b4a5968778695a4b3c2d1e0ff0e1d2c3b4a5968778695a4b3c2d1e0f");
  const payload = { v: 1, name: "GitHub", login: { username: "octocat", password: "hunter2" }, uris: ["https://github.com"] };
  const iv = (n: number) => new Uint8Array(12).fill(n);

  // Chunked blob: 2500 bytes in 1024-byte chunks -> 3 chunks, the last one flagged final.
  const blobKey = fromHex("aa".repeat(32));
  const plain = new Uint8Array(2500).map((_, i) => i % 251);
  const chunkSize = 1024;
  const parts: Buffer[] = [Buffer.concat([Buffer.from("MCB1"), Buffer.from([0, 0, 4, 0])])];
  const chunks = Math.ceil(plain.length / chunkSize);
  for (let i = 0; i < chunks; i++) {
    const aad = Buffer.alloc(9);
    aad.writeBigUInt64BE(BigInt(i));
    aad[8] = i === chunks - 1 ? 1 : 0;
    const chunkIv = iv(0x40 + i);
    parts.push(Buffer.from(chunkIv), gcm(blobKey, chunkIv, plain.subarray(i * chunkSize, (i + 1) * chunkSize), aad));
  }

  const sendKey = fromHex("0102030405060708090a0b0c0d0e0f10");
  const contentKey = hkdf(sendKey, "monica-cloud/v1/send");
  return {
    description: "Monica Cloud protocol v1 vectors. Byte strings are hex unless noted; EncStrings and authHash use standard base64.",
    kdf: { email, password, iterations, masterKey: hex(masterKey), authHash, wrapKey: hex(wrapKey) },
    userKey: { raw: hex(userKey), protectedUserKey: enc(wrapKey, iv(1), userKey) },
    item: {
      itemKey: hex(itemKey),
      key: enc(userKey, iv(2), itemKey),
      payload,
      data: enc(itemKey, iv(3), Buffer.from(JSON.stringify(payload)))
    },
    blob: { key: hex(blobKey), chunkSize, plaintext: hex(plain), ciphertext: hex(Buffer.concat(parts)) },
    send: {
      sendKey: hex(sendKey),
      sendKeyUrl: Buffer.from(sendKey).toString("base64url"),
      contentKey: hex(contentKey),
      text: "Wi-Fi: Guest-2026!",
      encryptedText: enc(contentKey, iv(4), Buffer.from("Wi-Fi: Guest-2026!")),
      password: "open sesame",
      passwordHash: b64(nodeCrypto.pbkdf2Sync(Buffer.from("open sesame"), sendKey, 100_000, 32, "sha256"))
    },
    totp: { secret: "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", digits: 8, period: 30, algorithm: "SHA1", unixTime: 59, code: "94287082" }
  };
}

async function verify(v: ReturnType<typeof generate>) {
  const keys = await mc.deriveMasterKeys(v.kdf.email, v.kdf.password, v.kdf.iterations);
  assert.equal(keys.authHash, v.kdf.authHash, "authHash");
  const userKey = await mc.unwrapUserKey(keys.wrapKey, v.userKey.protectedUserKey);
  assert.equal(hex(userKey.raw), v.userKey.raw, "userKey");
  const item = await mc.decryptItem(userKey.key, { key: v.item.key, data: v.item.data });
  assert.equal(hex(item.itemKeyRaw), v.item.itemKey, "itemKey");
  assert.deepEqual(item.payload, v.item.payload, "payload");
  const blob = await mc.decryptBlob(await mc.importAesKey(fromHex(v.blob.key)), fromHex(v.blob.ciphertext));
  assert.equal(hex(blob), v.blob.plaintext, "blob");
  const send = await mc.deriveSendKeys(mc.b64urlDecode(v.send.sendKeyUrl));
  assert.equal(await mc.decryptString(send.contentKey, v.send.encryptedText), v.send.text, "send text");
  assert.equal(await mc.sendPasswordHash(v.send.password, send.sendKey), v.send.passwordHash, "send password");
  const totp = parseTotp(v.totp.secret, { digits: v.totp.digits, period: v.totp.period, algorithm: v.totp.algorithm })!;
  assert.equal(await totpCode(totp, v.totp.unixTime * 1000), v.totp.code, "totp");
}

if (process.argv.includes("--write")) {
  const vectors = generate();
  await verify(vectors);
  fs.writeFileSync(out, JSON.stringify(vectors, null, 2) + "\n");
  console.log(`wrote ${out.pathname}`);
} else {
  await verify(JSON.parse(fs.readFileSync(out, "utf8")));
  console.log("test vectors verified");
}
