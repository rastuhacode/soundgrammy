import * as React from "react";
import { cn } from "@/lib/utils";

const sizeClasses = {
  default: "size-9",
  lg: "size-10",
} as const;

interface AvatarContextValue {
  imageLoaded: boolean;
  setImageLoaded: (loaded: boolean) => void;
}

const AvatarContext = React.createContext<AvatarContextValue | null>(null);

export function Avatar({
  className,
  size = "default",
  children,
}: {
  className?: string;
  size?: keyof typeof sizeClasses;
  children?: React.ReactNode;
}) {
  const [imageLoaded, setImageLoaded] = React.useState(false);
  return (
    <AvatarContext.Provider value={{ imageLoaded, setImageLoaded }}>
      <span
        className={cn(
          "relative flex shrink-0 overflow-hidden rounded-full",
          sizeClasses[size],
          className,
        )}
      >
        {children}
      </span>
    </AvatarContext.Provider>
  );
}

export function AvatarImage({
  src,
  alt,
  className,
}: {
  src?: string;
  alt?: string;
  className?: string;
}) {
  const ctx = React.useContext(AvatarContext);
  const [errored, setErrored] = React.useState(false);

  React.useEffect(() => {
    setErrored(false);
    ctx?.setImageLoaded(false);
  }, [src]);

  if (!src || errored) return null;

  return (
    <img
      src={src}
      alt={alt}
      className={cn("aspect-square size-full object-cover", className)}
      onLoad={() => ctx?.setImageLoaded(true)}
      onError={() => setErrored(true)}
    />
  );
}

export function AvatarFallback({
  className,
  children,
}: {
  className?: string;
  children?: React.ReactNode;
}) {
  const ctx = React.useContext(AvatarContext);
  if (ctx?.imageLoaded) return null;
  return (
    <span
      className={cn(
        "flex size-full items-center justify-center rounded-full bg-muted",
        className,
      )}
    >
      {children}
    </span>
  );
}
