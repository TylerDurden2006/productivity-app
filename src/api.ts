import { invoke } from "@tauri-apps/api/core";
import type { AppError } from "./generated/AppError";
import type { ErrorCode } from "./generated/ErrorCode";

export type StartupView = {
  ok: boolean;
  error: string | null;
  dbPath: string;
  backupsDir: string;
  newestBackup: string | null;
  dataDir: string;
  configuredDataDir: string;
  oldDataDir: string | null;
  dataWarning: string | null;
};

export type ActionPayload = Record<string, unknown>;

export async function getStartup(): Promise<StartupView> {
  return invoke<StartupView>("get_startup");
}

export async function runAction<T>(
  action: string,
  payload: ActionPayload = {},
): Promise<T> {
  return invoke<T>("run_action", { action, payload });
}

export function normalizeError(error: unknown): AppError {
  if (isAppError(error)) {
    return error;
  }
  if (error instanceof Error) {
    return {
      code: "INTERNAL" as ErrorCode,
      message: error.message,
      overridable: false,
      hint: null,
    };
  }
  if (typeof error === "string") {
    return {
      code: "INTERNAL" as ErrorCode,
      message: error,
      overridable: false,
      hint: null,
    };
  }
  return {
    code: "INTERNAL" as ErrorCode,
    message: "Something went wrong while saving. Your data is unchanged.",
    overridable: false,
    hint: null,
  };
}

export function isAppError(error: unknown): error is AppError {
  if (!error || typeof error !== "object") {
    return false;
  }
  const candidate = error as Partial<AppError>;
  return (
    typeof candidate.code === "string" &&
    typeof candidate.message === "string" &&
    typeof candidate.overridable === "boolean"
  );
}


export async function prepareDataMove(path: string): Promise<{ ok: boolean; target: string }> {
  return invoke("prepare_data_move", { path });
}

export async function cancelDataMove(): Promise<{ ok: boolean }> {
  return invoke("cancel_data_move");
}

export async function openDataFolder(): Promise<{ ok: boolean }> {
  return invoke("open_data_folder");
}

export async function restartApp(): Promise<void> {
  return invoke("restart_app");
}
