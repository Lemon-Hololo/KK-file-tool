export interface PreviewPayload {
  type: "text" | "image" | "archive_list" | "unsupported";
  [key: string]: any;
}