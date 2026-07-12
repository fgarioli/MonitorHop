import type { Configuration } from "./api";

export interface MainScreenProps {
  config: Configuration;
  onReconfigure: () => void;
}

// Temporary stub — replaced by Task 13's real main-screen implementation.
export function MainScreen(_props: MainScreenProps) {
  return null;
}
