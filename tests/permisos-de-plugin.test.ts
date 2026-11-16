import { describe, expect, it } from 'vitest';
import { revisar as forzados } from '../scripts/check-forzados.mjs';
import { revisar as permisos } from '../scripts/check-permisos.mjs';

describe('lo que la interfaz llama de un plugin tiene su permiso', () => {
  it('el árbol real no usa ninguna función de plugin sin conceder', () => {
    expect(permisos()).toEqual([]);
  });
});

describe('ningún forzado de tipo sin justificar', () => {
  it('el árbol real no tiene ninguno', () => {
    expect(forzados()).toEqual([]);
  });
});
