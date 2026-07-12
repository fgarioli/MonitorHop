import { useState } from "react";
import { listUsbDevices } from "../api";

interface Props {
  label: string;
  onSelected: (deviceId: string) => void;
  onSkip?: () => void;
}

/** "Plug it in, click the one that appeared": snapshots the USB device list,
 * asks the user to plug in the device, re-snapshots, and highlights whatever
 * is new. */
export function DeviceStep({ label, onSelected, onSkip }: Props) {
  const [before, setBefore] = useState<string[] | null>(null);
  const [candidates, setCandidates] = useState<string[]>([]);

  const snapshotBefore = async () => setBefore(await listUsbDevices());

  const detectNew = async () => {
    const after = await listUsbDevices();
    const beforeSet = new Set(before ?? []);
    setCandidates(after.filter((id) => !beforeSet.has(id)));
  };

  return (
    <div>
      <h2>{label}</h2>
      {before === null && (
        <div>
          <button onClick={snapshotBefore}>Start</button>
          {onSkip && <button onClick={onSkip}>Skip</button>}
        </div>
      )}
      {before !== null && candidates.length === 0 && (
        <div>
          <p>Now plug in the device (or unplug/replug it).</p>
          <button onClick={detectNew}>I plugged it in</button>
          {onSkip && <button onClick={onSkip}>Skip</button>}
        </div>
      )}
      {candidates.length > 0 && (
        <ul>
          {candidates.map((id) => (
            <li key={id}>
              {id} <button onClick={() => onSelected(id)}>Use this</button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
