// FlipaClip .fcp glue: wraps Rust flare_fcp_parse (JSON result).
export interface FcpSummary { fps: number; width: number; height: number; frames: string[] }
export type FcpParse = (data: Uint8Array) => string | null;
export function importFcp(parse: FcpParse, data: Uint8Array): FcpSummary {
  const j = parse(data);
  if (!j) throw new Error("not a valid .fcp");
  return JSON.parse(j) as FcpSummary;
}
