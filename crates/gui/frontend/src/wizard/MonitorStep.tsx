import { useEffect, useState } from "react";
import { listMonitors, MonitorInfo } from "../api";

interface Props {
  onSelected: (monitor: MonitorInfo) => void;
}

export function MonitorStep({ onSelected }: Props) {
  const [monitors, setMonitors] = useState<MonitorInfo[] | null>(null);

  useEffect(() => {
    listMonitors().then(setMonitors);
  }, []);

  if (monitors === null) return <p>Detecting monitors…</p>;
  if (monitors.length === 0) return <p>No DDC-compatible monitors detected.</p>;

  return (
    <div>
      <h2>Select the monitor this KVM setup controls</h2>
      <ul>
        {monitors.map((m) => (
          <li key={m.display_index}>
            {m.model_name ?? m.id} (display index {m.display_index})
            <button onClick={() => onSelected(m)}>Use this monitor</button>
          </li>
        ))}
      </ul>
    </div>
  );
}
