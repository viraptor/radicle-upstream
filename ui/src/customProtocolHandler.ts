import * as ipc from "./ipc";
import * as session from "./session";
import * as modal from "./modal";
import * as path from "./path";
import * as error from "./error";
import { inputStore } from "./search";

export const register = (): void => {
  ipc.listenCustomProtocolInvocation(async message => {
    await session.waitUnsealed();
    handleMessage(message);
  });
};

const handleMessage = (message: ipc.CustomProtocolInvocation) => {
  const projectId = parseUrl(message.url);

  if (projectId) {
    inputStore.set(projectId);
    modal.show(path.search());
  } else {
    error.show({
      code: error.Code.CustomProtocolParseError,
      message: "Could not extract project Radicle ID from the provided URL",
      details: { url: message.url },
    });
  }
};

const parseUrl = (url: string): string | null => {
  const match = url.match(/(rad:git:[1-9A-HJ-NP-Za-km-z]{37})/);

  if (match) {
    return match[1];
  } else {
    return null;
  }
};
