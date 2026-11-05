// Tipos espejo de los modelos Rust (src-tauri/src/app_core.rs y provider/mod.rs).
// Ninguno contiene el token; la clave solo llega con api.revealKey().

export type AuthStatus = "signedOut" | "pending" | "signedIn";

export type LiveAccess =
  | "approved"
  | "legacy"
  | "notApproved"
  | "neverApplied"
  | "rejected"
  | "relog"
  | "unknown";

export interface ErrorInfo {
  code: string;
  category: string;
  message: string;
}

export interface AudienceOption {
  value: string;
  label: string;
}

export interface AudienceControls {
  disabled: boolean;
  defaultValue: string;
  options: AudienceOption[];
}

export interface AccountInfo {
  username: string;
  displayName: string | null;
  liveAccess: LiveAccess;
  canBeLive: boolean | null;
  applicationStatus: string | null;
  applicationTimestamp: string | null;
  audience: AudienceControls;
  checkedAtMs: number;
}

export interface PendingView {
  loginUrl: string;
  browserOpened: boolean;
  startedAtMs: number;
  expiresAtMs: number;
}

export type Persistence =
  | { mode: "memory" }
  | { mode: "keyring" }
  | { mode: "unavailable"; detail: string };

export interface AuthView {
  status: AuthStatus;
  pending: PendingView | null;
  account: AccountInfo | null;
  accountError: ErrorInfo | null;
  remember: boolean;
  persistence: Persistence;
}

export interface StreamView {
  id: string;
  server: string | null;
  keyHint: string | null;
  keyAvailable: boolean;
  title: string;
  categoryName: string;
  startedAtMs: number;
  restored: boolean;
}

export interface Category {
  id: string;
  name: string;
}

export interface FormDefaults {
  title: string;
  category: Category | null;
  audienceType: string;
  titleMaxChars: number;
}

export interface AppInfo {
  version: string;
  os: string;
  devicePlatform: string;
  provider: string;
}

export interface Snapshot {
  auth: AuthView;
  stream: StreamView | null;
  busy: string | null;
  form: FormDefaults;
  app: AppInfo;
}

export interface StartLiveRequest {
  title: string;
  category: Category;
  audienceType: string;
}

export type CopyField = "server" | "key" | "loginUrl";
