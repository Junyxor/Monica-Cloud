/** RFC 6238 TOTP from a base32 secret or an otpauth:// URI, using WebCrypto. */

export interface TotpParams {
  secret: Uint8Array<ArrayBuffer>;
  digits: number;
  period: number;
  algorithm: "SHA-1" | "SHA-256" | "SHA-512";
}

const BASE32 = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

function base32Decode(input: string): Uint8Array<ArrayBuffer> | null {
  const clean = input.toUpperCase().replace(/[\s=-]/g, "");
  if (!clean || [...clean].some((c) => !BASE32.includes(c))) return null;
  const out: number[] = [];
  let bits = 0;
  let value = 0;
  for (const char of clean) {
    value = (value << 5) | BASE32.indexOf(char);
    bits += 5;
    if (bits >= 8) {
      out.push((value >>> (bits - 8)) & 0xff);
      bits -= 8;
    }
  }
  return new Uint8Array(out);
}

export function parseTotp(raw: string | undefined, defaults: Partial<{ digits: number; period: number; algorithm: string }> = {}): TotpParams | null {
  if (!raw?.trim()) return null;
  let secret = raw.trim();
  let digits = defaults.digits ?? 6;
  let period = defaults.period ?? 30;
  let algorithm = (defaults.algorithm ?? "SHA1").toUpperCase();
  if (secret.toLowerCase().startsWith("otpauth://")) {
    try {
      const url = new URL(secret);
      secret = url.searchParams.get("secret") ?? "";
      digits = Number(url.searchParams.get("digits") ?? digits);
      period = Number(url.searchParams.get("period") ?? period);
      algorithm = (url.searchParams.get("algorithm") ?? algorithm).toUpperCase();
    } catch {
      return null;
    }
  } else if (secret.toLowerCase().startsWith("steam://")) {
    return null; // Steam Guard codes use a different alphabet; shown by the app only.
  }
  const bytes = base32Decode(secret);
  if (!bytes || !bytes.length) return null;
  const algo = algorithm.replace("SHA", "SHA-") as TotpParams["algorithm"];
  return {
    secret: bytes,
    digits: digits >= 6 && digits <= 10 ? digits : 6,
    period: period >= 1 && period <= 300 ? period : 30,
    algorithm: ["SHA-1", "SHA-256", "SHA-512"].includes(algo) ? algo : "SHA-1"
  };
}

export async function totpCode(params: TotpParams, now = Date.now()): Promise<string> {
  const counter = Math.floor(now / 1000 / params.period);
  const message = new Uint8Array(8);
  new DataView(message.buffer).setBigUint64(0, BigInt(counter));
  const key = await crypto.subtle.importKey("raw", params.secret, { name: "HMAC", hash: params.algorithm }, false, ["sign"]);
  const hmac = new Uint8Array(await crypto.subtle.sign("HMAC", key, message));
  const offset = hmac[hmac.length - 1] & 0x0f;
  const binary =
    ((hmac[offset] & 0x7f) << 24) | (hmac[offset + 1] << 16) | (hmac[offset + 2] << 8) | hmac[offset + 3];
  return String(binary % 10 ** params.digits).padStart(params.digits, "0");
}

export function secondsRemaining(period: number, now = Date.now()): number {
  return period - (Math.floor(now / 1000) % period);
}
