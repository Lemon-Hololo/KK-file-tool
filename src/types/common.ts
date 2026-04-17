export type TaskStatus =
  | "Idle"
  | "Running"
  | "Paused"
  | "Cancelled"
  | "Completed"
  | "Failed";

export interface TaskLogPayload {
  taskId: string;
  level: "INFO" | "WARN" | "ERROR";
  message: string;
  filePath?: string;
}

export interface TaskProgressPayload {
  taskId: string;
  stage: string;
  processed: number;
  total: number;
  percent: number;
}
