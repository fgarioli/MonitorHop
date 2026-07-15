import { useEffect, useState } from "react";
import { ChevronLeft } from "lucide-react";
import { listInputs } from "../api";
import { vcpInputLabel } from "../vcpLabels";
import { InlineError } from "../components/InlineError";

interface Props {
  displayIndex: number;
  initialOnConnect?: number | null;
  initialOnDisconnect?: number | null;
  onBack?: () => void;
  onComplete: (mapping: { onConnect: number; onDisconnect: number | null }) => void;
}

export function InputMappingStep({
  displayIndex,
  initialOnConnect = null,
  initialOnDisconnect = null,
  onBack,
  onComplete,
}: Props) {
  const [inputs, setInputs] = useState<number[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [onConnect, setOnConnect] = useState<number | null>(initialOnConnect);
  const [onDisconnect, setOnDisconnect] = useState<number | null>(initialOnDisconnect);

  useEffect(() => {
    listInputs(displayIndex)
      .then(setInputs)
      .catch((err) => setError(String(err)));
  }, [displayIndex]);

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
          <button
            disabled={onConnect === null}
            onClick={() => onComplete({ onConnect: onConnect!, onDisconnect })}
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
