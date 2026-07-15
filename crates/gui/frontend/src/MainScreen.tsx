import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Keyboard, Monitor as MonitorIcon, Settings } from "lucide-react";
import { Configuration, currentInput, listInputs, switchInput } from "./api";
import { vcpInputLabel } from "./vcpLabels";
import { StatusBadge } from "./components/StatusBadge";
import { InlineError } from "./components/InlineError";

export function MainScreen({ config, onReconfigure }: { config: Configuration; onReconfigure: () => void }) {
  const [inputs, setInputs] = useState<number[]>([]);
  const [active, setActive] = useState<number | null>(null);
  const [mxkeysConnected, setMxkeysConnected] = useState<boolean | null>(null);
  const [error, setError] = useState<string | null>(null);

  const displayIndex = config.display_index ?? 0;

  useEffect(() => {
    listInputs(displayIndex)
      .then(setInputs)
      .catch((err) => setError(String(err)));
    currentInput(displayIndex)
      .then(setActive)
      .catch((err) => setError(String(err)));
  }, [displayIndex]);

  useEffect(() => {
    const unlisten = listen<boolean>("mxkeys-status", (event) => setMxkeysConnected(event.payload));
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  const handleSwitch = async (value: number) => {
    setError(null);
    try {
      await switchInput(value);
      setActive(value);
    } catch (err) {
      setError(String(err));
    }
  };

  return (
    <div className="flex h-full flex-col gap-5 p-5">
      <div className="flex items-center justify-between">
        <h1 className="text-lg font-semibold text-neutral-900 dark:text-neutral-100">KVM Switch</h1>
        <button
          onClick={onReconfigure}
          aria-label="Reconfigure"
          className="rounded-md p-2 text-neutral-500 hover:bg-neutral-100 dark:hover:bg-neutral-800"
        >
          <Settings size={18} />
        </button>
      </div>

      <div className="flex items-center gap-2">
        <Keyboard size={16} className="text-neutral-500" />
        <span className="text-sm text-neutral-600 dark:text-neutral-400">MX Keys receiver:</span>
        <StatusBadge status={mxkeysConnected === null ? "unknown" : mxkeysConnected ? "connected" : "disconnected"} />
      </div>

      <div className="flex flex-col gap-2">
        <h2 className="flex items-center gap-2 text-sm font-semibold text-neutral-700 dark:text-neutral-300">
          <MonitorIcon size={16} className="text-emerald-600" />
          Switch input
        </h2>
        <ul className="flex flex-col gap-2">
          {inputs.map((v) => (
            <li
              key={v}
              className={`flex items-center justify-between gap-3 rounded-md border px-3 py-2 ${
                v === active
                  ? "border-emerald-500 bg-emerald-50 dark:border-emerald-500 dark:bg-emerald-950/40"
                  : "border-neutral-200 bg-white dark:border-neutral-700 dark:bg-neutral-900"
              }`}
            >
              <span className="text-sm">{vcpInputLabel(v)}</span>
              <button
                disabled={v === active}
                onClick={() => handleSwitch(v)}
                className="rounded-md bg-emerald-600 px-3 py-1 text-sm font-medium text-white hover:bg-emerald-700 disabled:cursor-not-allowed disabled:opacity-50"
              >
                {v === active ? "Active" : "Switch"}
              </button>
            </li>
          ))}
        </ul>
      </div>

      <InlineError message={error} />
    </div>
  );
}
