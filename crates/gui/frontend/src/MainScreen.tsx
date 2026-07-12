import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Configuration, listInputs, switchInput } from "./api";

export function MainScreen({ config, onReconfigure }: { config: Configuration; onReconfigure: () => void }) {
  const [inputs, setInputs] = useState<number[]>([]);
  const [mxkeysConnected, setMxkeysConnected] = useState<boolean | null>(null);

  useEffect(() => {
    listInputs(config.display_index ?? 0).then(setInputs);
  }, [config.display_index]);

  useEffect(() => {
    const unlisten = listen<boolean>("mxkeys-status", (event) => setMxkeysConnected(event.payload));
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  const hex = (v: number) => `0x${v.toString(16).toUpperCase()}`;

  return (
    <div>
      <h1>KVM Switch</h1>
      <p>
        MX Keys receiver:{" "}
        {mxkeysConnected === null ? "unknown" : mxkeysConnected ? "connected on this host" : "not connected"}
      </p>
      <h2>Switch input</h2>
      <ul>
        {inputs.map((v) => (
          <li key={v}>
            {hex(v)} <button onClick={() => switchInput(v)}>Switch</button>
          </li>
        ))}
      </ul>
      <button onClick={onReconfigure}>Reconfigure</button>
    </div>
  );
}
