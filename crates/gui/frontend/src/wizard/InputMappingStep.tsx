import { useEffect, useState } from "react";
import { ChevronLeft } from "lucide-react";
import { listInputs } from "../api";
import { vcpInputLabel } from "../vcpLabels";
import { InlineError } from "../components/InlineError";

interface Props {
  displayIndex: number;
  initialOnConnect?: number | null;
  initialOnDisconnect?: number | null;
  initialSourceAddr?: number | null;
  initialVcpCode?: number | null;
  onBack?: () => void;
  onComplete: (mapping: {
    onConnect: number;
    onDisconnect: number | null;
    sourceAddr: number | null;
    vcpCode: number | null;
  }) => void;
}

/** Parses an optional hex byte the user typed (e.g. "0x50", "50", ""),
 * returning `null` for a blank/whitespace-only input and `undefined` if the
 * text doesn't parse to a valid 0-255 byte. Exported so its edge cases are
 * testable without mounting the component. */
export function parseHexByte(raw: string): number | null | undefined {
  const trimmed = raw.trim();
  if (trimmed === "") return null;
  const withoutPrefix = trimmed.toLowerCase().startsWith("0x") ? trimmed.slice(2) : trimmed;
  if (!/^[0-9a-f]+$/i.test(withoutPrefix)) return undefined;
  const value = parseInt(withoutPrefix, 16);
  return value >= 0 && value <= 0xff ? value : undefined;
}

export function InputMappingStep({
  displayIndex,
  initialOnConnect = null,
  initialOnDisconnect = null,
  initialSourceAddr = null,
  initialVcpCode = null,
  onBack,
  onComplete,
}: Props) {
  const [inputs, setInputs] = useState<number[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [onConnect, setOnConnect] = useState<number | null>(initialOnConnect);
  const [onDisconnect, setOnDisconnect] = useState<number | null>(initialOnDisconnect);
  const [sourceAddrText, setSourceAddrText] = useState(
    initialSourceAddr != null ? `0x${initialSourceAddr.toString(16)}` : "",
  );
  const [vcpCodeText, setVcpCodeText] = useState(
    initialVcpCode != null ? `0x${initialVcpCode.toString(16)}` : "",
  );

  useEffect(() => {
    listInputs(displayIndex)
      .then(setInputs)
      .catch((err) => setError(String(err)));
  }, [displayIndex]);

  const sourceAddr = parseHexByte(sourceAddrText);
  const vcpCode = parseHexByte(vcpCodeText);
  const advancedFieldsInvalid = sourceAddr === undefined || vcpCode === undefined;

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center gap-2">
        {onBack && (
          <button
            onClick={onBack}
            aria-label="Back"
            className="rounded-md p-1 text-neutral-500 hover:bg-neutral-100 dark:hover:bg-neutral-800"
          >
            <ChevronLeft size={18} />
          </button>
        )}
        <h2 className="text-base font-semibold text-neutral-900 dark:text-neutral-100">Map inputs</h2>
      </div>

      {inputs === null && !error && (
        <p className="text-sm text-neutral-600 dark:text-neutral-400">Reading supported inputs…</p>
      )}

      {inputs !== null && (
        <>
          <label className="flex flex-col gap-1 text-sm text-neutral-700 dark:text-neutral-300">
            Switch to this input when the KVM switch connects to this host:
            <select
              value={onConnect ?? ""}
              onChange={(e) => setOnConnect(e.target.value ? Number(e.target.value) : null)}
              className="rounded-md border border-neutral-300 px-2 py-1.5 text-sm dark:border-neutral-700 dark:bg-neutral-900"
            >
              <option value="">Select…</option>
              {inputs.map((v) => (
                <option key={v} value={v}>
                  {vcpInputLabel(v)}
                </option>
              ))}
            </select>
          </label>
          <label className="flex flex-col gap-1 text-sm text-neutral-700 dark:text-neutral-300">
            Switch to this input on disconnect (optional):
            <select
              value={onDisconnect ?? ""}
              onChange={(e) => setOnDisconnect(e.target.value ? Number(e.target.value) : null)}
              className="rounded-md border border-neutral-300 px-2 py-1.5 text-sm dark:border-neutral-700 dark:bg-neutral-900"
            >
              <option value="">None</option>
              {inputs.map((v) => (
                <option key={v} value={v}>
                  {vcpInputLabel(v)}
                </option>
              ))}
            </select>
          </label>

          <div className="flex flex-col gap-2 rounded-md border border-neutral-200 p-2 dark:border-neutral-700">
            <span className="text-xs font-medium uppercase tracking-wide text-neutral-500">
              Advanced (optional — only if your monitor needs a non-standard address)
            </span>
            <label className="flex flex-col gap-1 text-sm text-neutral-700 dark:text-neutral-300">
              I2C source-address override (hex; Windows only, ignored on macOS). Blank uses this
              app's default, 0x50.
              <input
                type="text"
                placeholder="0x50"
                value={sourceAddrText}
                onChange={(e) => setSourceAddrText(e.target.value)}
                className="rounded-md border border-neutral-300 px-2 py-1.5 text-sm dark:border-neutral-700 dark:bg-neutral-900"
              />
            </label>
            <label className="flex flex-col gap-1 text-sm text-neutral-700 dark:text-neutral-300">
              VCP feature-code override (hex). Blank uses the DDC/CI standard, 0x60.
              <input
                type="text"
                placeholder="0x60"
                value={vcpCodeText}
                onChange={(e) => setVcpCodeText(e.target.value)}
                className="rounded-md border border-neutral-300 px-2 py-1.5 text-sm dark:border-neutral-700 dark:bg-neutral-900"
              />
            </label>
            {advancedFieldsInvalid && (
              <p role="alert" className="text-sm text-red-600 dark:text-red-400">
                Enter a valid hex byte (00–FF), or leave the field blank for the default.
              </p>
            )}
          </div>

          <button
            disabled={onConnect === null || advancedFieldsInvalid}
            onClick={() =>
              onComplete({
                onConnect: onConnect!,
                onDisconnect,
                sourceAddr: sourceAddr ?? null,
                vcpCode: vcpCode ?? null,
              })
            }
            className="self-start rounded-md bg-emerald-600 px-4 py-2 text-sm font-medium text-white hover:bg-emerald-700 disabled:cursor-not-allowed disabled:opacity-50"
          >
            Finish
          </button>
        </>
      )}

      <InlineError message={error} />
    </div>
  );
}
