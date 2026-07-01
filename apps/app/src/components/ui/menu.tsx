import * as React from "react";
import { cn } from "@/lib/utils";

interface MenuContextValue {
  open: boolean;
  setOpen: (open: boolean) => void;
  triggerRef: React.RefObject<HTMLElement | null>;
}

const MenuContext = React.createContext<MenuContextValue | null>(null);

function useMenu() {
  const ctx = React.useContext(MenuContext);
  if (!ctx) throw new Error("Menu components must be used within <Menu>");
  return ctx;
}

export function Menu({ children }: { children: React.ReactNode }) {
  const [open, setOpen] = React.useState(false);
  const triggerRef = React.useRef<HTMLElement | null>(null);
  return (
    <MenuContext.Provider value={{ open, setOpen, triggerRef }}>
      <div className="relative inline-flex">{children}</div>
    </MenuContext.Provider>
  );
}

export function MenuTrigger({ children }: { children: React.ReactElement }) {
  const { open, setOpen, triggerRef } = useMenu();
  const child = children as React.ReactElement<
    Record<string, unknown> & { ref?: React.Ref<HTMLElement> }
  >;
  return React.cloneElement(child, {
    ref: triggerRef,
    "aria-expanded": open,
    onClick: (event: React.MouseEvent) => {
      event.stopPropagation();
      (child.props.onClick as ((e: React.MouseEvent) => void) | undefined)?.(
        event,
      );
      setOpen(!open);
    },
  });
}

export function MenuContent({
  className,
  align = "start",
  sideOffset = 6,
  children,
}: {
  className?: string;
  align?: "start" | "end" | "center";
  sideOffset?: number;
  children: React.ReactNode;
}) {
  const { open, setOpen, triggerRef } = useMenu();
  const contentRef = React.useRef<HTMLDivElement>(null);

  React.useEffect(() => {
    if (!open) return;
    const onPointer = (e: MouseEvent) => {
      const target = e.target as Node;
      if (
        contentRef.current?.contains(target) ||
        triggerRef.current?.contains(target)
      ) {
        return;
      }
      setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onPointer);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onPointer);
      document.removeEventListener("keydown", onKey);
    };
  }, [open, setOpen, triggerRef]);

  if (!open) return null;

  const alignClass =
    align === "end"
      ? "right-0"
      : align === "center"
        ? "left-1/2 -translate-x-1/2"
        : "left-0";

  return (
    <div
      ref={contentRef}
      role="menu"
      style={{ top: `calc(100% + ${sideOffset}px)` }}
      className={cn(
        "absolute z-100 min-w-40 overflow-hidden rounded-xl border border-border bg-popover p-1.5 text-popover-foreground shadow-lg animate-fade-up",
        alignClass,
        className,
      )}
      onClick={(e) => e.stopPropagation()}
    >
      {children}
    </div>
  );
}

interface MenuItemProps
  extends Omit<React.ButtonHTMLAttributes<HTMLButtonElement>, "onSelect"> {
  variant?: "default" | "destructive";
  inset?: boolean;
}

export function MenuItem({
  className,
  variant = "default",
  disabled,
  onClick,
  children,
  ...props
}: MenuItemProps) {
  const { setOpen } = useMenu();
  return (
    <button
      type="button"
      role="menuitem"
      disabled={disabled}
      className={cn(
        "flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-left text-sm transition-colors outline-none",
        "focus-visible:bg-muted/70",
        variant === "destructive"
          ? "text-destructive hover:bg-destructive/10"
          : "text-foreground hover:bg-muted/70",
        disabled && "pointer-events-none opacity-50",
        "[&_svg]:size-4 [&_svg]:shrink-0",
        className,
      )}
      onClick={(event) => {
        event.stopPropagation();
        onClick?.(event);
        if (!event.defaultPrevented) setOpen(false);
      }}
      {...props}
    >
      {children}
    </button>
  );
}

export function MenuLabel({
  className,
  children,
}: {
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <div
      className={cn(
        "px-2 py-1.5 font-mono text-[10px] uppercase tracking-[0.16em] text-muted-foreground",
        className,
      )}
    >
      {children}
    </div>
  );
}

export function MenuSeparator({ className }: { className?: string }) {
  return <div className={cn("my-1.5 h-px bg-border/70", className)} />;
}
