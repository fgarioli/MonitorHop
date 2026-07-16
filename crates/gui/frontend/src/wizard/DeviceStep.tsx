import { useEffect, useState } from "react";
import { ChevronLeft, Usb } from "lucide-react";
import { listUsbDevices, loadDeviceDatabase } from "../api";
import { usbDeviceLabel } from "../usbVendorLabels";
import { InlineError } from "../components/InlineError";

interface Props {
  label: string;
  onSelected: (deviceId: string) => void;
  onSkip?: () => void;
  onBack?: () => void;
}

/** Primary view: every USB device connected right now, labeled via the
 * runtime device database, directly selectable — no unplug/replug
 * required (see
 * docs/superpowers/specs/2026-07-15-immediate-device-detection-design.md).
 * `DiffDetectionFlow` below stays available as a secondary, collapsed
 * fallback for a genuinely new, uncataloged device. */
export function DeviceStep({ label, onSelected, onSkip, onBack }: Props) {
  const [connected, setConnected] = useState<string[] | null>(null);
  const [database, setDatabase] = useState<Record<string, string>>({});
  const [error, setError] = useState<string | null>(null);
  const [showDiffFlow, setShowDiffFlow] = useState(false);

  useEffect(() => {
    listUsbDevices()
      .then(setConnected)
      .catch((err) => setError(String(err)));
    loadDeviceDatabase()
      .then(setDatabase)
      .catch(() => {
        // A failed name lookup degrades to raw ids, not a blocking error —
        // the device list itself comes from listUsbDevices() above, which
        // has its own independent error handling.
      });
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
          <Usb size={18} className="text-emerald-600" />
          {label}
        </h2>
      </div>

      {connected === null && !error && (
        <p className="text-sm text-neutral-600 dark:text-neutral-400">Detecting connected devices…</p>
      )}

      {connected !== null && (
        <ul className="flex flex-col gap-2">
          {connected.map((id) => (
            <li
              key={id}
              className="flex items-center justify-between gap-3 rounded-md border border-neutral-200 bg-white px-3 py-2 dark:border-neutral-700 dark:bg-neutral-900"
            >
              <span className="text-sm">{usbDeviceLabel(id, database)}</span>
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

      {onSkip && (
        <button
          onClick={onSkip}
          className="self-start rounded-md border border-neutral-300 px-4 py-2 text-sm font-medium text-neutral-700 hover:bg-neutral-100 dark:border-neutral-700 dark:text-neutral-200 dark:hover:bg-neutral-800"
        >
          Skip
        </button>
      )}

      <InlineError message={error} />

      {!showDiffFlow && (
        <button
          onClick={() => setShowDiffFlow(true)}
          className="self-start text-sm text-neutral-500 underline hover:text-neutral-700 dark:text-neutral-400 dark:hover:text-neutral-200"
        >
          Not sure which one? Plug it in now
        </button>
      )}

      {showDiffFlow && <DiffDetectionFlow existingIds={connected ?? []} onSelected={onSelected} />}
    </div>
  );
}

/** The original "plug it in, click the one that appeared" flow, kept as a
 * fallback for a device not recognizable from the direct connected-device
 * list above. Snapshots against `existingIds` (the list `DeviceStep`
 * already fetched on mount) instead of taking its own fresh "before"
 * snapshot, so revealing this flow needs no extra network round-trip. */
function DiffDetectionFlow({ existingIds, onSelected }: { existingIds: string[]; onSelected: (deviceId: string) => void }) {
  const [candidates, setCandidates] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [checked, setChecked] = useState(false);

  const detectNew = async () => {
    setError(null);
    try {
      const after = await listUsbDevices();
      const beforeSet = new Set(existingIds);
      setCandidates(after.filter((id) => !beforeSet.has(id)));
      setChecked(true);
    } catch (err) {
      setError(String(err));
    }
  };

  return (
    <div className="flex flex-col gap-2 border-t border-neutral-200 pt-3 dark:border-neutral-700">
      <p className="text-sm text-neutral-600 dark:text-neutral-400">
        Plug in (or unplug and replug) the device, then click below.
      </p>
      <button
        onClick={detectNew}
        className="self-start rounded-md border border-neutral-300 px-4 py-2 text-sm font-medium text-neutral-700 hover:bg-neutral-100 dark:border-neutral-700 dark:text-neutral-200 dark:hover:bg-neutral-800"
      >
        I plugged it in
      </button>

      {checked && candidates.length === 0 && (
        <p className="text-sm text-neutral-600 dark:text-neutral-400">
          No new device detected — try unplugging and replugging it.
        </p>
      )}

      {candidates.length > 0 && (
        <ul className="flex flex-col gap-2">
          {candidates.map((id) => (
            <li
              key={id}
              className="flex items-center justify-between gap-3 rounded-md border border-neutral-200 bg-white px-3 py-2 dark:border-neutral-700 dark:bg-neutral-900"
            >
              <span className="text-sm">{id}</span>
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
