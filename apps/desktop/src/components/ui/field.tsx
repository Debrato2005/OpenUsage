"use client";

import "./selection.css";

import { Field as FieldPrimitive } from "@base-ui/react/field";
import type React from "react";
import { cn } from "../../lib/utils";

export function Field({
  className,
  ...props
}: FieldPrimitive.Root.Props): React.ReactElement {
  return (
    <FieldPrimitive.Root
      className={(state) => cn("coss-field flex flex-col items-start gap-2", typeof className === "function" ? className(state) : className)}
      data-slot="field"
      {...props}
    />
  );
}

export function FieldLabel({
  className,
  ...props
}: FieldPrimitive.Label.Props): React.ReactElement {
  return (
    <FieldPrimitive.Label
      className={(state) => cn(
        "coss-field-label inline-flex items-center gap-2 font-medium text-base/4.5 text-foreground data-disabled:opacity-64 sm:text-sm/4",
        typeof className === "function" ? className(state) : className,
      )}
      data-slot="field-label"
      {...props}
    />
  );
}

export function FieldItem({
  className,
  ...props
}: FieldPrimitive.Item.Props): React.ReactElement {
  return (
    <FieldPrimitive.Item
      className={(state) => cn("coss-field-item flex", typeof className === "function" ? className(state) : className)}
      data-slot="field-item"
      {...props}
    />
  );
}

export function FieldDescription({
  className,
  ...props
}: FieldPrimitive.Description.Props): React.ReactElement {
  return (
    <FieldPrimitive.Description
      className={(state) => cn("coss-field-description text-muted-foreground text-xs", typeof className === "function" ? className(state) : className)}
      data-slot="field-description"
      {...props}
    />
  );
}

export function FieldError({
  className,
  ...props
}: FieldPrimitive.Error.Props): React.ReactElement {
  return (
    <FieldPrimitive.Error
      className={(state) => cn("coss-field-error text-destructive-foreground text-xs", typeof className === "function" ? className(state) : className)}
      data-slot="field-error"
      {...props}
    />
  );
}

export const FieldControl: typeof FieldPrimitive.Control =
  FieldPrimitive.Control;
export const FieldValidity: typeof FieldPrimitive.Validity =
  FieldPrimitive.Validity;

export { FieldPrimitive };
