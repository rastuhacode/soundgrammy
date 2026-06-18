"use client";

import { useCallback, useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { useMutation, useQuery } from "@tanstack/react-query";
import { Button } from "@/components/ui/button";
import styles from "@/app/login/page.module.css";
import { useTRPC } from "../trpc/client";

type Step = "qr" | "phone" | "code" | "password" | "qr-password";

function errorMessage(error: unknown, fallback: string): string {
  return error instanceof Error ? error.message : fallback;
}

export function MtprotoLogin() {
  const router = useRouter();
  const trpc = useTRPC();

  const [step, setStep] = useState<Step>("qr");
  const [qrDataUrl, setQrDataUrl] = useState<string | null>(null);
  const [qrAuthToken, setQrAuthToken] = useState<string | null>(null);
  const [phoneNumber, setPhoneNumber] = useState("");
  const [code, setCode] = useState("");
  const [password, setPassword] = useState("");
  const [authToken, setAuthToken] = useState<string | null>(null);
  const [codeDelivery, setCodeDelivery] = useState<"app" | "sms">("app");
  const [passwordHint, setPasswordHint] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const { mutateAsync: startQr, isPending: qrStarting } = useMutation(
    trpc.mtproto.qr.start.mutationOptions(),
  );
  const { mutateAsync: submitQrPassword, isPending: qrPasswordPending } =
    useMutation(trpc.mtproto.qr.password.mutationOptions());
  const { mutateAsync: sendCode, isPending: sendingCode } = useMutation(
    trpc.mtproto.sendCode.mutationOptions(),
  );
  const { mutateAsync: resendCode, isPending: resending } = useMutation(
    trpc.mtproto.resendCode.mutationOptions(),
  );
  const { mutateAsync: signIn, isPending: signingIn } = useMutation(
    trpc.mtproto.signIn.mutationOptions(),
  );
  const { mutateAsync: submitPassword, isPending: passwordPending } =
    useMutation(trpc.mtproto.password.mutationOptions());

  const startQrLogin = useCallback(async () => {
    setError(null);
    setQrDataUrl(null);
    try {
      const data = await startQr();
      setQrAuthToken(data.authToken);
      setQrDataUrl(data.qrDataUrl ?? null);
      setStep("qr");
    } catch (err) {
      setError(errorMessage(err, "Failed to start QR login"));
    }
  }, [startQr]);

  useEffect(() => {
    void startQrLogin();
  }, [startQrLogin]);

  const qrStatusQuery = useQuery({
    ...trpc.mtproto.qr.status.queryOptions({ authToken: qrAuthToken ?? "" }),
    enabled: Boolean(qrAuthToken) && (step === "qr" || step === "qr-password"),
    refetchInterval: 2000,
  });

  useEffect(() => {
    const data = qrStatusQuery.data;
    if (!data) return;

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
  }, [qrStatusQuery.data, router]);

  const handleSendCode = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    try {
      const data = await sendCode({ phoneNumber });
      setAuthToken(data.authToken);
      setCodeDelivery(data.codeDelivery ?? "app");
      setStep("code");
    } catch (err) {
      setError(errorMessage(err, "Failed to send code"));
    }
  };

  const handleResendSms = async () => {
    if (!authToken) return;
    setError(null);
    try {
      const data = await resendCode({ authToken });
      setCodeDelivery(data.codeDelivery ?? "sms");
    } catch (err) {
      setError(errorMessage(err, "Failed to resend code"));
    }
  };

  const handleSignIn = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!authToken) return;
    setError(null);
    try {
      const data = await signIn({ authToken, code });
      if (data.needsPassword) {
        setStep("password");
        return;
      }
      router.push("/");
      router.refresh();
    } catch (err) {
      setError(errorMessage(err, "Failed to sign in"));
    }
  };

  const handlePhonePassword = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!authToken) return;
    setError(null);
    try {
      await submitPassword({ authToken, password });
      router.push("/");
      router.refresh();
    } catch (err) {
      setError(errorMessage(err, "Invalid password"));
    }
  };

  const handleQrPassword = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!qrAuthToken) return;
    setError(null);
    try {
      await submitQrPassword({ authToken: qrAuthToken, password });
      setStep("qr");
    } catch (err) {
      setError(errorMessage(err, "Invalid password"));
    }
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
                    {qrStarting ? "Generating QR code…" : "Loading QR code…"}
                  </p>
                )}
                <p className="text-xs opacity-50 text-center">
                  Works without SMS — recommended for Russian numbers
                </p>
                <Button
                  type="button"
                  variant="ghost"
                  onClick={() => void startQrLogin()}
                  disabled={qrStarting}
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
                <Button type="submit" disabled={qrPasswordPending} className="w-full">
                  {qrPasswordPending ? "Signing in…" : "Continue"}
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
            <Button type="submit" disabled={sendingCode} className="w-full">
              {sendingCode ? "Sending…" : "Send login code"}
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
            <Button type="submit" disabled={signingIn} className="w-full">
              {signingIn ? "Verifying…" : "Sign in"}
            </Button>
            {codeDelivery === "app" ? (
              <Button
                type="button"
                variant="outline"
                disabled={resending}
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
            <Button type="submit" disabled={passwordPending} className="w-full">
              {passwordPending ? "Signing in…" : "Sign in"}
            </Button>
          </form>
        )}
      </div>
    </div>
  );
}
