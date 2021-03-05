// Messages sent from the main process to the renderer
export type MainMessage =
  | {
      kind: MainMessageKind.PROXY_ERROR;
      data: ProxyError;
    }
  | {
      kind: MainMessageKind.CUSTOM_PROTOCOL_INVOCATION;
      data: CustomProtocolInvocation;
    };

export enum MainMessageKind {
  PROXY_ERROR = "PROXY_ERROR",
  CUSTOM_PROTOCOL_INVOCATION = "CUSTOM_PROTOCOL_INVOCATION",
}

// Payload for the ProxyError `MainMessage`.
export interface ProxyError {
  status: number | null;
  signal: NodeJS.Signals | null;
  output: string;
}

// Payload for the CustomProtocolInvocation `MainMessage`
export interface CustomProtocolInvocation {
  url: string;
}

// Message kinds sent from the renderer to the main process.
export enum RendererMessage {
  CLIPBOARD_WRITETEXT = "IPC_CLIPBOARD_WRITETEXT",
  DIALOG_SHOWOPENDIALOG = "IPC_DIALOG_SHOWOPENDIALOG",
  GET_VERSION = "GET_VERSION",
  OPEN_PATH = "IPC_OPEN_PATH",
  OPEN_URL = "IPC_OPEN_URL",
  GET_GIT_GLOBAL_DEFAULT_BRANCH = "GET_GIT_GLOBAL_DEFAULT_BRANCH",
}
