import { useState } from "react";
import { ChevronLeft, Usb } from "lucide-react";
import { listUsbDevices } from "../api";
import { usbDeviceLabel } from "../usbVendorLabels";
import { InlineError } from "../components/InlineError";

interface Props {
  label: string;
  onSelected: (deviceId: string) => void;
  onSkip?: () => void;
  onBack?: () => void;
}

/** "Plug it in, click the one that appeared": snapshots the USB device list,
 * asks the user to plug in the device, re-snapshots, and highlights whatever
 * is new. */
export function DeviceStep({ label, onSelected, onSkip, onBack }: Props) {
  const [before, setBefore] = useState<string[] | null>(null);
  const [candidates, setCandidates] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);

  const snapshotBefore = async () => {
    setError(null);
    try {
      setBefore(await listUsbDevices());
    } catch (err) {
      setError(String(err));
    }
  };

  const detectNew = async () => {
    setError(null);
    try {
      const after = await listUsbDevices();
      const beforeSet = new Set(before ?? []);
      setCandidates(after.filter((id) => !beforeSet.has(id)));
    } catch (err) {
      setError(String(err));
    }
  };

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
          <Usb size={18} className="text-emerald-600" />
          {label}
        </h2>
      </div>

      {before === null && (
        <div className="flex gap-2">
          <button
            onClick={snapshotBefore}
            className="rounded-md bg-emerald-600 px-4 py-2 text-sm font-medium text-white hover:bg-emerald-700"
          >
            Start
          </button>
          {onSkip && (
            <button
              onClick={onSkip}
              className="rounded-md border border-neutral-300 px-4 py-2 text-sm font-medium text-neutral-700 hover:bg-neutral-100 dark:border-neutral-700 dark:text-neutral-200 dark:hover:bg-neutral-800"
            >
              Skip
            </button>
          )}
        </div>
      )}

      {before !== null && candidates.length === 0 && (
        <div className="flex flex-col gap-2">
          <p className="text-sm text-neutral-600 dark:text-neutral-400">
            Now plug in the device (or unplug/replug it).
          </p>
          <div className="flex gap-2">
            <button
              onClick={detectNew}
              className="rounded-md bg-emerald-600 px-4 py-2 text-sm font-medium text-white hover:bg-emerald-700"
            >
              I plugged it in
            </button>
            {onSkip && (
              <button
                onClick={onSkip}
                className="rounded-md border border-neutral-300 px-4 py-2 text-sm font-medium text-neutral-700 hover:bg-neutral-100 dark:border-neutral-700 dark:text-neutral-200 dark:hover:bg-neutral-800"
              >
                Skip
              </button>
            )}
          </div>
        </div>
      )}

      {candidates.length > 0 && (
        <ul className="flex flex-col gap-2">
          {candidates.map((id) => (
            <li
              key={id}
              className="flex items-center justify-between gap-3 rounded-md border border-neutral-200 bg-white px-3 py-2 dark:border-neutral-700 dark:bg-neutral-900"
            >
              <span className="text-sm">{usbDeviceLabel(id)}</span>
              <button
                onClick={() => onSelected(id)}
                className="rounded-md bg-emerald-600 px-3 py-1 text-sm font-medium text-white hover:bg-emerald-700"
              >
                Use this
              </button>
            </li>
          ))}
        </ul>
      )}

      <InlineError message={error} />
    </div>
  );
}
