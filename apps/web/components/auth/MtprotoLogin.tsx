"use client";

import { useCallback, useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { useMutation, useQuery } from "@tanstack/react-query";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useTRPC } from "@/trpc/client";

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
  const { mutateAsync: submitQrPassword, isPending: qrPasswordPending }
    = useMutation(trpc.mtproto.qr.password.mutationOptions());
  const { mutateAsync: sendCode, isPending: sendingCode } = useMutation(
    trpc.mtproto.sendCode.mutationOptions(),
  );
  const { mutateAsync: resendCode, isPending: resending } = useMutation(
    trpc.mtproto.resendCode.mutationOptions(),
  );
  const { mutateAsync: signIn, isPending: signingIn } = useMutation(
    trpc.mtproto.signIn.mutationOptions(),
  );
  const { mutateAsync: submitPassword, isPending: passwordPending }
    = useMutation(trpc.mtproto.password.mutationOptions());

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
    <div className="hifi-bg relative flex min-h-screen items-center justify-center overflow-hidden px-5 py-12">
      <div className="pointer-events-none absolute left-1/2 top-[-10%] h-144 w-xl -translate-x-1/2 rounded-full bg-primary/15 blur-[120px] animate-glow-pulse" />

      <div className="relative w-full max-w-104 animate-fade-up">
        <div className="mb-8 text-center">
          <span
            aria-hidden
            className="equalizer mx-auto mb-5 h-7 justify-center [&>span]:w-1"
          >
            <span />
            <span />
            <span />
            <span />
          </span>
          <h1 className="font-serif text-4xl font-semibold tracking-tight text-foreground text-glow">
            Sound
            <span className="text-primary">grammy</span>
          </h1>
          <p className="mx-auto mt-3 max-w-xs text-sm leading-relaxed text-muted-foreground">
            Sign in with Telegram to tune in your profile music.
          </p>
        </div>

        <div className="rounded-2xl border border-border bg-card/80 p-7 shadow-[0_24px_60px_-24px_rgba(0,0,0,0.7)] backdrop-blur-xl">
          {error
            ? (
                <p
                  className="mb-5 rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2 text-center text-sm text-destructive"
                  role="alert"
                >
                  {error}
                </p>
              )
            : null}

          {step === "qr" || step === "qr-password"
            ? (
                <div className="flex w-full flex-col items-center gap-4">
                  {step === "qr"
                    ? (
                        <>
                          <p className="text-center text-sm leading-relaxed text-muted-foreground">
                            Scan with Telegram on your phone
                            <br />
                            <span className="font-mono text-xs uppercase tracking-wide text-foreground/70">
                              Settings → Devices → Link Desktop
                            </span>
                          </p>
                          <div className="relative rounded-xl border border-border bg-foreground p-3">
                            {qrDataUrl
                              ? (
                                  <img
                                    src={qrDataUrl}
                                    alt="Telegram login QR code"
                                    width={232}
                                    height={232}
                                    className="rounded-md"
                                  />
                                )
                              : (
                                  <div className="flex h-[232px] w-[232px] items-center justify-center">
                                    <span className="font-mono text-xs text-background/60">
                                      {qrStarting ? "Generating…" : "Loading…"}
                                    </span>
                                  </div>
                                )}
                          </div>
                          <p className="text-center text-xs text-muted-foreground/80">
                            Works without SMS — recommended for Russian numbers
                          </p>
                          <Button
                            type="button"
                            variant="ghost"
                            size="sm"
                            onClick={() => void startQrLogin()}
                            disabled={qrStarting}
                          >
                            Refresh QR code
                          </Button>
                        </>
                      )
                    : (
                        <form
                          onSubmit={(e) => void handleQrPassword(e)}
                          className="flex w-full flex-col gap-3"
                        >
                          <p className="text-center text-sm text-muted-foreground">
                            2FA enabled
                            {passwordHint ? `: ${passwordHint}` : ""}
                          </p>
                          <Input
                            type="password"
                            placeholder="2FA password"
                            value={password}
                            onChange={(e) => setPassword(e.target.value)}
                            required
                          />
                          <Button
                            type="submit"
                            disabled={qrPasswordPending}
                            className="w-full"
                          >
                            {qrPasswordPending ? "Signing in…" : "Continue"}
                          </Button>
                        </form>
                      )}
                  <div className="h-px w-full bg-border" />
                  <Button
                    type="button"
                    variant="outline"
                    className="w-full"
                    onClick={() => setStep("phone")}
                  >
                    Use phone number instead
                  </Button>
                </div>
              )
            : step === "phone"
              ? (
                  <form
                    onSubmit={(e) => void handleSendCode(e)}
                    className="flex w-full flex-col gap-3"
                  >
                    <p className="text-center text-xs leading-relaxed text-muted-foreground">
                      Phone login may not work for Russian numbers (SMS blocked).
                      Prefer QR login.
                    </p>
                    <Input
                      type="tel"
                      placeholder="+79991234567"
                      value={phoneNumber}
                      onChange={(e) => setPhoneNumber(e.target.value)}
                      required
                    />
                    <Button type="submit" disabled={sendingCode} className="w-full">
                      {sendingCode ? "Sending…" : "Send login code"}
                    </Button>
                    <Button
                      type="button"
                      variant="ghost"
                      onClick={() => setStep("qr")}
                    >
                      Back to QR login
                    </Button>
                  </form>
                )
              : step === "code"
                ? (
                    <form
                      onSubmit={(e) => void handleSignIn(e)}
                      className="flex w-full flex-col gap-3"
                    >
                      <p className="text-center text-sm leading-relaxed text-muted-foreground">
                        {codeDelivery === "sms"
                          ? "Check SMS for your login code."
                          : "Check the Telegram app chat with \"Telegram\" for your code."}
                      </p>
                      <Input
                        type="text"
                        inputMode="numeric"
                        placeholder="Login code"
                        value={code}
                        onChange={(e) => setCode(e.target.value)}
                        required
                        className="text-center font-mono text-lg tracking-[0.5em]"
                      />
                      <Button type="submit" disabled={signingIn} className="w-full">
                        {signingIn ? "Verifying…" : "Sign in"}
                      </Button>
                      {codeDelivery === "app"
                        ? (
                            <Button
                              type="button"
                              variant="outline"
                              disabled={resending}
                              onClick={() => void handleResendSms()}
                            >
                              Try SMS instead
                            </Button>
                          )
                        : null}
                      <Button
                        type="button"
                        variant="ghost"
                        onClick={() => setStep("qr")}
                      >
                        Back to QR login
                      </Button>
                    </form>
                  )
                : (
                    <form
                      onSubmit={(e) => void handlePhonePassword(e)}
                      className="flex w-full flex-col gap-3"
                    >
                      <Input
                        type="password"
                        placeholder="2FA password"
                        value={password}
                        onChange={(e) => setPassword(e.target.value)}
                        required
                      />
                      <Button type="submit" disabled={passwordPending} className="w-full">
                        {passwordPending ? "Signing in…" : "Sign in"}
                      </Button>
                    </form>
                  )}
        </div>

        <p className="mt-6 text-center font-mono text-[10px] uppercase tracking-[0.3em] text-muted-foreground/60">
          End-to-end via MTProto
        </p>
      </div>
    </div>
  );
}
