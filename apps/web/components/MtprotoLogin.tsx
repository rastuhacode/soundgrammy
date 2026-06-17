"use client";

import { useCallback, useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { Button } from "@/components/ui/button";
import styles from "@/app/login/page.module.css";

type Step = "qr" | "phone" | "code" | "password" | "qr-password";
type CodeDelivery = "app" | "sms";

export function MtprotoLogin() {
  const router = useRouter();
  const [step, setStep] = useState<Step>("qr");
  const [qrDataUrl, setQrDataUrl] = useState<string | null>(null);
  const [qrAuthToken, setQrAuthToken] = useState<string | null>(null);
  const [phoneNumber, setPhoneNumber] = useState("");
  const [code, setCode] = useState("");
  const [password, setPassword] = useState("");
  const [authToken, setAuthToken] = useState<string | null>(null);
  const [codeDelivery, setCodeDelivery] = useState<CodeDelivery>("app");
  const [passwordHint, setPasswordHint] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const startQrLogin = useCallback(async () => {
    setLoading(true);
    setError(null);
    setQrDataUrl(null);

    const response = await fetch("/api/mtproto/auth/qr/start", {
      method: "POST",
    });
    const data = await response.json();
    setLoading(false);

    if (!response.ok) {
      setError(data.error ?? "Failed to start QR login");
      return;
    }

    setQrAuthToken(data.authToken);
    setQrDataUrl(data.qrDataUrl ?? null);
    setStep("qr");
  }, []);

  useEffect(() => {
    void startQrLogin();
  }, [startQrLogin]);

  useEffect(() => {
    if (step !== "qr" && step !== "qr-password") return;
    if (!qrAuthToken) return;

    const interval = setInterval(async () => {
      const response = await fetch(
        `/api/mtproto/auth/qr/status?authToken=${encodeURIComponent(qrAuthToken)}`,
      );
      const data = await response.json();

      if (data.status === "success") {
        router.push("/");
        router.refresh();
        return;
      }

      if (data.status === "awaiting_password") {
        setPasswordHint(data.passwordHint ?? null);
        setStep("qr-password");
        return;
      }

      if (data.status === "error") {
        setError(data.error ?? "QR login failed");
      }
    }, 2000);

    return () => clearInterval(interval);
  }, [step, qrAuthToken, router]);

  const handleSendCode = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setError(null);

    const response = await fetch("/api/mtproto/auth/send-code", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ phoneNumber }),
    });

    const data = await response.json();
    setLoading(false);

    if (!response.ok) {
      setError(data.error ?? "Failed to send code");
      return;
    }

    setAuthToken(data.authToken);
    setCodeDelivery(data.codeDelivery ?? "app");
    setStep("code");
  };

  const handleResendSms = async () => {
    if (!authToken) return;

    setLoading(true);
    setError(null);

    const response = await fetch("/api/mtproto/auth/resend-code", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ authToken }),
    });

    const data = await response.json();
    setLoading(false);

    if (!response.ok) {
      setError(data.error ?? "Failed to resend code");
      return;
    }

    setCodeDelivery(data.codeDelivery ?? "sms");
  };

  const handleSignIn = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!authToken) return;

    setLoading(true);
    setError(null);

    const response = await fetch("/api/mtproto/auth/sign-in", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ authToken, code }),
    });

    const data = await response.json();
    setLoading(false);

    if (!response.ok) {
      setError(data.error ?? "Failed to sign in");
      return;
    }

    if (data.needsPassword) {
      setStep("password");
      return;
    }

    router.push("/");
    router.refresh();
  };

  const handlePhonePassword = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!authToken) return;

    setLoading(true);
    setError(null);

    const response = await fetch("/api/mtproto/auth/password", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ authToken, password }),
    });

    const data = await response.json();
    setLoading(false);

    if (!response.ok) {
      setError(data.error ?? "Invalid password");
      return;
    }

    router.push("/");
    router.refresh();
  };

  const handleQrPassword = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!qrAuthToken) return;

    setLoading(true);
    setError(null);

    const response = await fetch("/api/mtproto/auth/qr/password", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ authToken: qrAuthToken, password }),
    });

    const data = await response.json();
    setLoading(false);

    if (!response.ok) {
      setError(data.error ?? "Invalid password");
      return;
    }

    setStep("qr");
  };

  return (
    <div className={styles.container}>
      <div className={styles.card}>
        <div className={styles.logo}>SoundGrammy</div>
        <p className={styles.subtitle}>
          Sign in with Telegram to sync profile music
        </p>

        {error ? (
          <p className="text-sm text-destructive w-full text-center" role="alert">
            {error}
          </p>
        ) : null}

        {step === "qr" || step === "qr-password" ? (
          <div className="flex flex-col gap-3 w-full items-center">
            {step === "qr" ? (
              <>
                <p className="text-sm opacity-80 text-center">
                  Scan with Telegram on your phone:
                  <br />
                  <span className="opacity-70">Settings → Devices → Link Desktop Device</span>
                </p>
                {qrDataUrl ? (
                  <img
                    src={qrDataUrl}
                    alt="Telegram login QR code"
                    width={280}
                    height={280}
                    className="rounded-lg bg-white"
                  />
                ) : (
                  <p className="text-sm opacity-60">
                    {loading ? "Generating QR code…" : "Loading QR code…"}
                  </p>
                )}
                <p className="text-xs opacity-50 text-center">
                  Works without SMS — recommended for Russian numbers
                </p>
                <Button
                  type="button"
                  variant="ghost"
                  onClick={() => void startQrLogin()}
                  disabled={loading}
                >
                  Refresh QR code
                </Button>
              </>
            ) : (
              <form
                onSubmit={(e) => void handleQrPassword(e)}
                className="flex flex-col gap-3 w-full"
              >
                <p className="text-sm opacity-80 text-center">
                  2FA enabled{passwordHint ? `: ${passwordHint}` : ""}
                </p>
                <input
                  type="password"
                  placeholder="2FA password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  className="h-10 rounded-md border border-input bg-background px-3 text-sm w-full"
                  required
                />
                <Button type="submit" disabled={loading} className="w-full">
                  {loading ? "Signing in…" : "Continue"}
                </Button>
              </form>
            )}
            <Button
              type="button"
              variant="outline"
              onClick={() => setStep("phone")}
            >
              Use phone number instead
            </Button>
          </div>
        ) : step === "phone" ? (
          <form
            onSubmit={(e) => void handleSendCode(e)}
            className="flex flex-col gap-3 w-full"
          >
            <p className="text-xs opacity-60 text-center">
              Phone login may not work for Russian numbers (SMS blocked). Prefer QR login.
            </p>
            <input
              type="tel"
              placeholder="+79991234567"
              value={phoneNumber}
              onChange={(e) => setPhoneNumber(e.target.value)}
              className="h-10 rounded-md border border-input bg-background px-3 text-sm w-full"
              required
            />
            <Button type="submit" disabled={loading} className="w-full">
              {loading ? "Sending…" : "Send login code"}
            </Button>
            <Button type="button" variant="ghost" onClick={() => setStep("qr")}>
              Back to QR login
            </Button>
          </form>
        ) : step === "code" ? (
          <form
            onSubmit={(e) => void handleSignIn(e)}
            className="flex flex-col gap-3 w-full"
          >
            <p className="text-sm opacity-80 text-center">
              {codeDelivery === "sms"
                ? "Check SMS for your login code."
                : "Check the Telegram app chat with \"Telegram\" for your code."}
            </p>
            <input
              type="text"
              inputMode="numeric"
              placeholder="Login code"
              value={code}
              onChange={(e) => setCode(e.target.value)}
              className="h-10 rounded-md border border-input bg-background px-3 text-sm w-full"
              required
            />
            <Button type="submit" disabled={loading} className="w-full">
              {loading ? "Verifying…" : "Sign in"}
            </Button>
            {codeDelivery === "app" ? (
              <Button
                type="button"
                variant="outline"
                disabled={loading}
                onClick={() => void handleResendSms()}
              >
                Try SMS instead
              </Button>
            ) : null}
            <Button type="button" variant="ghost" onClick={() => setStep("qr")}>
              Back to QR login
            </Button>
          </form>
        ) : (
          <form
            onSubmit={(e) => void handlePhonePassword(e)}
            className="flex flex-col gap-3 w-full"
          >
            <input
              type="password"
              placeholder="2FA password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              className="h-10 rounded-md border border-input bg-background px-3 text-sm w-full"
              required
            />
            <Button type="submit" disabled={loading} className="w-full">
              {loading ? "Signing in…" : "Sign in"}
            </Button>
          </form>
        )}
      </div>
    </div>
  );
}
