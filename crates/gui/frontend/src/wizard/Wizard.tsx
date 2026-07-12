import type { Configuration } from "../api";

export interface WizardProps {
  onComplete: (config: Configuration) => void;
}

// Temporary stub — replaced by Task 12's real wizard implementation.
export function Wizard(_props: WizardProps) {
  return null;
}
