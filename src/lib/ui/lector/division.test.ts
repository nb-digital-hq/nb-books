/**
 * **Criterio de aceptación:** el reparto y el cambio de mitad activa
 * se hacen solo con teclado, cada mitad guarda su progreso por separado, y en la
 * clase compacta la división no se ofrece.
 *
 * El reparto es aritmética a propósito: arrastrar un separador es un gesto de
 * puntero, y una acción exclusiva del ratón está prohibida.
 */

import { describe, expect, it } from 'vitest';

import {
  abrirEnSecundaria,
  alternarDivision,
  alternarFoco,
  anchos,
  cabeLaDivision,
  DIVISION_INICIAL,
  REPARTO_MAXIMO,
  REPARTO_MINIMO,
  repartir
} from './division';

describe('la vista dividida', () => {
  it('no se ofrece donde no cabe', () => {
    // Dos columnas de lectura en una ventana estrecha no son dos columnas de
    // lectura: son dos columnas ilegibles. Se omite, no se muestra rota.
    expect(cabeLaDivision('compacto')).toBe(false);
    expect(cabeLaDivision('medio')).toBe(true);
    expect(cabeLaDivision('amplio')).toBe(true);
  });

  it('el separador se mueve a pasos y no se sale de los límites', () => {
    let division = DIVISION_INICIAL;
    for (let i = 0; i < 40; i += 1) division = repartir(division, 1);
    expect(division.reparto).toBe(REPARTO_MAXIMO);

    for (let i = 0; i < 80; i += 1) division = repartir(division, -1);
    expect(division.reparto).toBe(REPARTO_MINIMO);
  });

  it('el reparto no acumula error al ir y volver', () => {
    // Sin redondear, sumar y restar el mismo paso deja 0.5000000000000001, y el
    // ancho de la mitad acaba siendo un número con quince decimales.
    const ida = repartir(repartir(DIVISION_INICIAL, 1), 1);
    const vuelta = repartir(repartir(ida, -1), -1);
    expect(vuelta.reparto).toBe(DIVISION_INICIAL.reparto);
  });

  it('el foco pasa de una mitad a la otra', () => {
    const segunda = alternarFoco(DIVISION_INICIAL);
    expect(segunda.foco).toBe('secundaria');
    expect(alternarFoco(segunda).foco).toBe('principal');
  });

  it('cerrar la división devuelve el foco a la mitad que queda', () => {
    // Dejarlo en una mitad que ya no existe haría que las teclas siguientes no
    // fueran a ninguna parte.
    const abierta = { ...alternarDivision(DIVISION_INICIAL), foco: 'secundaria' as const };
    const cerrada = alternarDivision(abierta);

    expect(cerrada.activa).toBe(false);
    expect(cerrada.foco).toBe('principal');
    expect(cerrada.secundario).toBeNull();
  });

  it('abrir un documento en la segunda mitad la activa y le da el foco', () => {
    const division = abrirEnSecundaria(DIVISION_INICIAL, 'archivo-2');

    expect(division.activa).toBe(true);
    expect(division.secundario).toBe('archivo-2');
    expect(division.foco).toBe('secundaria');
  });

  it('los anchos suman siempre el ancho entero', () => {
    for (const pasos of [-6, -1, 0, 1, 6]) {
      const { principal, secundaria } = anchos(repartir(DIVISION_INICIAL, pasos));
      expect(Number.parseInt(principal, 10) + Number.parseInt(secundaria, 10)).toBe(100);
    }
  });
});
