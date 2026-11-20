/**
 * Los autores de una tarjeta, abreviados cuando son muchos.
 *
 * Una antología firmada por treinta y cinco autores los enseñaba todos en la
 * tarjeta, cortados a media palabra, cuando lo que dice algo es "Varios". Con
 * hasta tres se enseñan enteros; con más, los dos primeros y cuántos quedan. La
 * lista entera sigue en la ficha.
 */
export function autoresBreves(autores: string | null | undefined, masTexto: string): string {
  if (!autores) return '';
  const lista = autores
    .split(',')
    .map((a) => a.trim())
    .filter(Boolean);
  if (lista.length <= 3) return lista.join(', ');
  return `${lista.slice(0, 2).join(', ')} ${masTexto.replace('{n}', String(lista.length - 2))}`;
}
