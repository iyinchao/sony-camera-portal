import type { InputHTMLAttributes } from 'react'
import { cn } from '@/lib/utils'

export function Input({ className, ...props }: InputHTMLAttributes<HTMLInputElement>) {
  return (
    <input
      className={cn(
        'flex h-11 w-full rounded-lg border border-border bg-background px-3.5 text-sm text-foreground transition',
        'placeholder:text-muted-foreground/70',
        'outline-none focus-visible:border-accent focus-visible:ring-2 focus-visible:ring-accent/25',
        'disabled:cursor-not-allowed disabled:opacity-50',
        className,
      )}
      {...props}
    />
  )
}
