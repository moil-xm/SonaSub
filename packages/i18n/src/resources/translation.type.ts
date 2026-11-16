export type TranslationShape<T> = {
  [Key in keyof T]: T[Key] extends string
    ? string
    : T[Key] extends object
      ? TranslationShape<T[Key]>
      : never;
};
