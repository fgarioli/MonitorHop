import { useEffect, useState } from "react";
import { ChevronLeft, Check, Monitor as MonitorIcon } from "lucide-react";
import { listMonitors, MonitorInfo } from "../api";
import { InlineError } from "../components/InlineError";

interface Props {
  initialSelection?: MonitorInfo | null;
  onSelected: (monitor: MonitorInfo) => void;
  onBack?: () => void;
}

export function MonitorStep({ initialSelection, onSelected, onBack }: Props) {
  const [monitors, setMonitors] = useState<MonitorInfo[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    listMonitors()
      .then(setMonitors)
      .catch((err) => setError(String(err)));
  }, []);

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
        <h2 className="flex items-center gap-2 text-base font-semibold text-neutral-900 dark:text-neutral-100">
          <MonitorIcon size={18} className="text-emerald-600" />
          Select the monitor this KVM setup controls
        </h2>
      </div>

      {monitors === null && !error && (
        <p className="text-sm text-neutral-600 dark:text-neutral-400">Detecting monitors…</p>
      )}
      {monitors !== null && monitors.length === 0 && (
        <p className="text-sm text-neutral-600 dark:text-neutral-400">
          No DDC-compatible monitors detected.
        </p>
      )}
      {monitors !== null && monitors.length > 0 && (
        <ul className="flex flex-col gap-2">
          {monitors.map((m) => {
            const isPrevious = initialSelection?.display_index === m.display_index;
            return (
              <li
                key={m.display_index}
                className="flex items-center justify-between gap-3 rounded-md border border-neutral-200 bg-white px-3 py-2 dark:border-neutral-700 dark:bg-neutral-900"
              >
                <span className="flex items-center gap-2 text-sm">
                  {isPrevious && <Check size={14} className="text-emerald-600" />}
                  {m.model_name ?? m.id} (display index {m.display_index})
                </span>
                <button
                  onClick={() => onSelected(m)}
                  className="rounded-md bg-emerald-600 px-3 py-1 text-sm font-medium text-white hover:bg-emerald-700"
                >
                  Use this monitor
                </button>
              </li>
            );
          })}
        </ul>
      )}

      <InlineError message={error} />
    </div>
  );
}
