export type ChatGptAccountStatus = {
  state: "signedOut" | "signedIn" | "reauthRequired";
  account: { registrationId: string; name: string | null; email: string | null; planGranted: boolean } | null;
  registrations: { id: string; name: string | null; email: string | null }[];
  signInPending: boolean;
};

export const signedOut: ChatGptAccountStatus = {
  state: "signedOut", account: null, registrations: [], signInPending: false,
};
