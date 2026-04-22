import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export type UpdateStatus =
  | { state: "idle" }
  | { state: "checking" }
  | { state: "uptodate" }
  | { state: "available"; update: Update }
  | { state: "downloading"; downloaded: number; total: number | null }
  | { state: "ready" }
  | { state: "error"; message: string };

export async function checkForUpdate(): Promise<UpdateStatus> {
  try {
    const update = await check();
    if (update) {
      return { state: "available", update };
    }
    return { state: "uptodate" };
  } catch (e) {
    return { state: "error", message: String(e) };
  }
}

export async function downloadAndInstall(
  update: Update,
  onProgress: (downloaded: number, total: number | null) => void,
): Promise<void> {
  let downloaded = 0;
  let contentLength: number | null = null;

  await update.downloadAndInstall((event) => {
    if (event.event === "Started") {
      contentLength = event.data.contentLength ?? null;
      onProgress(0, contentLength);
    } else if (event.event === "Progress") {
      downloaded += event.data.chunkLength;
      onProgress(downloaded, contentLength);
    }
  });

  await relaunch();
}
