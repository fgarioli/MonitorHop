import { useEffect, useState } from "react";
import { listInputs } from "../api";

interface Props {
  displayIndex: number;
  onComplete: (mapping: { onConnect: number; onDisconnect: number | null }) => void;
}

export function InputMappingStep({ displayIndex, onComplete }: Props) {
  const [inputs, setInputs] = useState<number[] | null>(null);
  const [onConnect, setOnConnect] = useState<number | null>(null);
  const [onDisconnect, setOnDisconnect] = useState<number | null>(null);

  useEffect(() => {
    listInputs(displayIndex).then(setInputs);
  }, [displayIndex]);

  if (inputs === null) return <p>Reading supported inputs…</p>;

  const hex = (v: number) => `0x${v.toString(16).toUpperCase()}`;

  return (
    <div>
      <h2>Map inputs</h2>
      <label>
        Switch to this input when the KVM switch connects to this host:
        <select onChange={(e) => setOnConnect(Number(e.target.value))}>
          <option value="">Select…</option>
          {inputs.map((v) => (
            <option key={v} value={v}>
              {hex(v)}
            </option>
          ))}
        </select>
      </label>
      <label>
        Switch to this input on disconnect (optional):
        <select onChange={(e) => setOnDisconnect(e.target.value ? Number(e.target.value) : null)}>
          <option value="">None</option>
          {inputs.map((v) => (
            <option key={v} value={v}>
              {hex(v)}
            </option>
          ))}
        </select>
      </label>
      <button disabled={onConnect === null} onClick={() => onComplete({ onConnect: onConnect!, onDisconnect })}>
        Finish
      </button>
    </div>
  );
}
