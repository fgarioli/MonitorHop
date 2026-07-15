import styles from "./StatusBadge.module.css";

type Status = "connected" | "disconnected" | "unknown";

const LABELS: Record<Status, string> = {
  connected: "Connected",
  disconnected: "Not connected",
  unknown: "Unknown",
};

export function StatusBadge({ status }: { status: Status }) {
  return (
    <span className={`${styles.badge} ${styles[status]}`}>
      <span className={styles.dot} />
      {LABELS[status]}
    </span>
  );
}
