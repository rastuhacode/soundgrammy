import { describe, expect, it } from 'vitest'
import {
  codeLoginSchema,
  passwordLoginSchema,
  phoneLoginSchema,
} from './login-schemas'

describe('phoneLoginSchema', () => {
  it('accepts and trims an international E.164 phone number', () => {
    expect(phoneLoginSchema.parse({ phoneNumber: '  +79991234567  ' })).toEqual({
      phoneNumber: '+79991234567',
    })
  })

  it.each([
    '79991234567',
    '+09991234567',
    '+123456',
    '+1234567890123456',
    '+1 (999) 123-45-67',
  ])('rejects invalid phone number %s', (phoneNumber) => {
    expect(phoneLoginSchema.safeParse({ phoneNumber }).success).toBe(false)
  })
})

describe('codeLoginSchema', () => {
  it('accepts and trims a five-digit Telegram code', () => {
    expect(codeLoginSchema.parse({ code: ' 12345 ' })).toEqual({ code: '12345' })
  })

  it.each(['', '1234', '123456', '12a45'])('rejects invalid code %s', (code) => {
    expect(codeLoginSchema.safeParse({ code }).success).toBe(false)
  })
})

describe('passwordLoginSchema', () => {
  it('requires a password without changing its contents', () => {
    expect(passwordLoginSchema.safeParse({ password: '' }).success).toBe(false)
    expect(passwordLoginSchema.parse({ password: ' pass phrase ' })).toEqual({
      password: ' pass phrase ',
    })
  })
})
