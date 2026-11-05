// Tipos espejo de los modelos Rust (src-tauri/src/app_core.rs y provider/mod.rs).
// Ninguno contiene el token; la clave solo llega con api.revealKey().

export type AuthStatus = "signedOut" | "pending" | "signedIn";

/** Campo de la respuesta: presente, ausente/nulo o con tipo inesperado. */
export type Field<T> =
  | { state: "present"; value: T }
  | { state: "missing" }
  | { state: "invalid"; found: string };

export type LiveScope =
  | "approved"
  | "legacy"
  | "notApproved"
  | "neverApplied"
  | "rejected"
  | "relog"
  | "pending"
  | "unknown";

export type UnavailableCause =
  | "notApproved"
  | "neverApplied"
  | "rejected"
  | "legacy"
  | "relog"
  | "cannotGoLive"
  | "pending";

export type Availability =
  | { kind: "confirmed" }
  | { kind: "notAvailable"; cause: UnavailableCause }
  | { kind: "contradictory"; detail: string }
  | { kind: "unknown"; detail: string };

export interface ErrorInfo {
  code: string;
  category: string;
  message: string;
  /** Mensaje original del proveedor (saneado). */
  detail: string | null;
  httpStatus: number | null;
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

export interface AccessCheck {
  reason: Field<number>;
  canBeLive: Field<boolean>;
  applicationStatus: Field<string>;
  userPresent: boolean;
  scope: LiveScope;
  availability: Availability;
}

export interface AccountInfo {
  username: string | null;
  displayName: string | null;
  access: AccessCheck;
  audience: AudienceControls;
  httpStatus: number;
  durationMs: number;
  checkedAtMs: number;
}

/** Estado de la comprobación del permiso (separado de la sesión). */
export type AccessView =
  | { state: "idle" }
  | { state: "checking" }
  | { state: "done"; account: AccountInfo }
  | { state: "failed"; error: ErrorInfo };

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
  username: string | null;
  access: AccessView;
  sessionError: ErrorInfo | null;
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
export type AccessPage = "apply" | "status";
