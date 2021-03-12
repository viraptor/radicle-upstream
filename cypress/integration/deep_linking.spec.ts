import * as commands from "../support/commands";
import * as ipcStub from "../support/ipc-stub";
import * as ipcTypes from "../../native/ipc-types";

context("deep linking", () => {
  beforeEach(() => {
    commands.resetProxyState();
    commands.onboardUser();
    cy.visit("./public/index.html");
  });

  context("when passing in a valid URI", () => {
    it("opens the search modal and pre-fills the input field with the Radicle ID", () => {
      ipcStub.getStubs().then(stubs => {
        stubs.sendMessage({
          kind: ipcTypes.MainMessageKind.CUSTOM_PROTOCOL_INVOCATION,
          data: {
            url:
              "radicle://v0/link/rad:git:hwd1yredna5k7undw9xurpm6mtfyczodz4fkute7bcpii3jb9uoj7tf1sho?action=show",
          },
        });
      });

      commands
        .pick("search-modal", "search-input")
        .should(
          "have.value",
          "rad:git:hwd1yredna5k7undw9xurpm6mtfyczodz4fkute7bcpii3jb9uoj7tf1sho"
        );
      commands
        .pick("search-modal", "follow-toggle")
        .should("contain", "Follow");
    });
  });

  context("when passing in an invalid URI", () => {
    it("shows an error notification", () => {
      ipcStub.getStubs().then(stubs => {
        stubs.sendMessage({
          kind: ipcTypes.MainMessageKind.CUSTOM_PROTOCOL_INVOCATION,
          data: {
            url:
              "radicle://v0/link/rad:git:THIS_IS_NOT_A_VALID_URN?action=show",
          },
        });
      });

      commands
        .pick("notification")
        .contains("Could not extract project Radicle ID from the provided URL");
    });
  });
});
