import { z } from 'zod'

export const phoneLoginSchema = z.object({
  phoneNumber: z
    .string()
    .trim()
    .regex(
      /^\+[1-9]\d{6,14}$/,
      'Enter a phone number in international format, e.g. +7 (999) 123-45-67',
    ),
})

export const codeLoginSchema = z.object({
  code: z
    .string()
    .trim()
    .regex(/^\d{5}$/, 'Enter the 5-digit login code from Telegram'),
})

export const passwordLoginSchema = z.object({
  password: z.string().min(1, 'Enter your 2FA password'),
})

export type PhoneLoginValues = z.infer<typeof phoneLoginSchema>
export type CodeLoginValues = z.infer<typeof codeLoginSchema>
export type PasswordLoginValues = z.infer<typeof passwordLoginSchema>

export function firstIssueMessage(
  error: z.ZodError,
  fallback = 'Invalid input',
): string {
  return error.issues[0]?.message ?? fallback
}
