import { useState } from "react";
import { Configuration, MonitorInfo, saveConfig } from "../api";
import { DeviceStep } from "./DeviceStep";
import { MonitorStep } from "./MonitorStep";
import { InputMappingStep } from "./InputMappingStep";
import { ProgressBar } from "../components/ProgressBar";
import { InlineError } from "../components/InlineError";
import styles from "./Wizard.module.css";

interface WizardAnswers {
  switchDevice: string | null;
  mxkeysDevice: string | null;
  monitor: MonitorInfo | null;
  onConnect: number | null;
  onDisconnect: number | null;
}

const STEP_COUNT = 4;

const emptyAnswers: WizardAnswers = {
  switchDevice: null,
  mxkeysDevice: null,
  monitor: null,
  onConnect: null,
  onDisconnect: null,
};

export function Wizard({ onComplete }: { onComplete: (config: Configuration) => void }) {
  const [stepIndex, setStepIndex] = useState(0);
  const [answers, setAnswers] = useState<WizardAnswers>(emptyAnswers);
  const [saveError, setSaveError] = useState<string | null>(null);

  const goBack = () => setStepIndex((i) => Math.max(0, i - 1));

  const finish = async (onConnect: number, onDisconnect: number | null, monitor: MonitorInfo) => {
    const config: Configuration = {
      usb_device: answers.switchDevice!,
      mxkeys_usb_device: answers.mxkeysDevice || null,
      on_usb_connect: `0x${onConnect.toString(16)}`,
      on_usb_disconnect: onDisconnect !== null ? `0x${onDisconnect.toString(16)}` : null,
      on_usb_connect_source_addr: null,
      on_usb_connect_vcp_code: null,
      display_index: monitor.display_index,
    };
    try {
      setSaveError(null);
      await saveConfig(config);
      onComplete(config);
    } catch (err) {
      setSaveError(String(err));
    }
  };

  return (
    <div className="mx-auto flex h-full max-w-md flex-col gap-4 p-5">
      <ProgressBar step={stepIndex + 1} total={STEP_COUNT} />
      <div key={stepIndex} className={styles.stepTransition}>
        {stepIndex === 0 && (
          <DeviceStep
            key="switch-device"
            label="Select the KVM switch USB device"
            onSelected={(id) => {
              setAnswers((a) => ({ ...a, switchDevice: id }));
              setStepIndex(1);
            }}
          />
        )}
        {stepIndex === 1 && (
          <DeviceStep
            key="mxkeys-device"
            label="Select the MX Keys receiver (optional — plug it in, or skip)"
            onSelected={(id) => {
              setAnswers((a) => ({ ...a, mxkeysDevice: id }));
              setStepIndex(2);
            }}
            onSkip={() => {
              setAnswers((a) => ({ ...a, mxkeysDevice: "" }));
              setStepIndex(2);
            }}
            onBack={goBack}
          />
        )}
        {stepIndex === 2 && (
          <MonitorStep
            initialSelection={answers.monitor}
            onSelected={(monitor) => {
              setAnswers((a) => ({ ...a, monitor }));
              setStepIndex(3);
            }}
            onBack={goBack}
          />
        )}
        {stepIndex === 3 && answers.monitor && (
          <InputMappingStep
            displayIndex={answers.monitor.display_index}
            initialOnConnect={answers.onConnect}
            initialOnDisconnect={answers.onDisconnect}
            onBack={goBack}
            onComplete={({ onConnect, onDisconnect }) => {
              setAnswers((a) => ({ ...a, onConnect, onDisconnect }));
              finish(onConnect, onDisconnect, answers.monitor!);
            }}
          />
        )}
      </div>
      <InlineError message={saveError} />
    </div>
  );
}
