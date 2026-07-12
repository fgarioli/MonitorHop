import { useState } from "react";
import { Configuration, MonitorInfo, saveConfig } from "../api";
import { DeviceStep } from "./DeviceStep";
import { MonitorStep } from "./MonitorStep";
import { InputMappingStep } from "./InputMappingStep";

type Step =
  | { name: "switch-device" }
  | { name: "mxkeys-device"; switchDevice: string }
  | { name: "monitor"; switchDevice: string; mxkeysDevice: string }
  | { name: "inputs"; switchDevice: string; mxkeysDevice: string; monitor: MonitorInfo };

export function Wizard({ onComplete }: { onComplete: (config: Configuration) => void }) {
  const [step, setStep] = useState<Step>({ name: "switch-device" });

  if (step.name === "switch-device") {
    return <DeviceStep label="Select the KVM switch USB device" onSelected={(id) => setStep({ name: "mxkeys-device", switchDevice: id })} />;
  }
  if (step.name === "mxkeys-device") {
    return (
      <DeviceStep
        label="Select the MX Keys receiver (optional — plug it in, or skip)"
        onSelected={(id) => setStep({ name: "monitor", switchDevice: step.switchDevice, mxkeysDevice: id })}
      />
    );
  }
  if (step.name === "monitor") {
    return (
      <MonitorStep
        onSelected={(monitor) =>
          setStep({ name: "inputs", switchDevice: step.switchDevice, mxkeysDevice: step.mxkeysDevice, monitor })
        }
      />
    );
  }

  return (
    <InputMappingStep
      displayIndex={step.monitor.display_index}
      onComplete={async ({ onConnect, onDisconnect }) => {
        const config: Configuration = {
          usb_device: step.switchDevice,
          mxkeys_usb_device: step.mxkeysDevice || null,
          on_usb_connect: `0x${onConnect.toString(16)}`,
          on_usb_disconnect: onDisconnect !== null ? `0x${onDisconnect.toString(16)}` : null,
          on_usb_connect_source_addr: null,
          on_usb_connect_vcp_code: null,
          display_index: step.monitor.display_index,
        };
        await saveConfig(config);
        onComplete(config);
      }}
    />
  );
}
