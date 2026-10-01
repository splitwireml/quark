<script lang="ts">
  import type { Snippet } from 'svelte';
  import Button from '../atoms/Button.svelte';
  import FormulaButtonGroup from './FormulaButtonGroup.svelte';
  import { quoteLiteral, type FormulaKind } from '../../lib/mutation-sql';

  type Props = {
    onInsert: (value: string) => void;
    onWrap: (prefix: string, suffix: string) => void;
    onTemplate: (prefix: string, middle: string, suffix: string) => void;
    onReplace: (value: string) => void;
    lead?: Snippet;
    conditional?: Snippet;
    keypad?: boolean;
    kind?: FormulaKind;
    ready?: boolean;
    editable?: boolean;
    activeFunction?: string;
  };

  let { onInsert, onWrap, onTemplate, onReplace, lead, conditional, keypad = false, kind, ready = true, editable = true, activeFunction = '' }: Props = $props();
  function enabled(...kinds: FormulaKind[]) { return !keypad || (ready && kinds.includes(kind ?? 'unknown')); }
  let comparable = $derived(!keypad || (ready && kind !== 'unknown'));
</script>

<div class="helper-groups" class:keypad>
  {@render lead?.()}
  <FormulaButtonGroup label="Arithmetic" {keypad} available={keypad && enabled('number')}>
    <Button type="button" disabled={!enabled('number')} onclick={() => onInsert(' + ')}>+</Button><Button type="button" disabled={!enabled('number')} onclick={() => onInsert(' - ')}>−</Button><Button type="button" disabled={!enabled('number')} onclick={() => onInsert(' * ')}>×</Button><Button type="button" disabled={!enabled('number')} onclick={() => onInsert(' / ')}>÷</Button>
    <Button type="button" disabled={!ready} onclick={() => onWrap('(', ')')}>( )</Button><Button type="button" disabled={!enabled('number')} onclick={() => onWrap('(', ') / 100')}>%</Button>
  </FormulaButtonGroup>
  <FormulaButtonGroup label="Compare" {keypad} wide={keypad} available={keypad && ready}>
    <Button type="button" disabled={!comparable} onclick={() => onInsert(' = ')}>==</Button><Button type="button" disabled={!comparable} onclick={() => onInsert(' <> ')}>!=</Button><Button type="button" disabled={!comparable} onclick={() => onInsert(' > ')}>&gt;</Button><Button type="button" disabled={!comparable} onclick={() => onInsert(' >= ')}>≥</Button><Button type="button" disabled={!comparable} onclick={() => onInsert(' < ')}>&lt;</Button><Button type="button" disabled={!comparable} onclick={() => onInsert(' <= ')}>≤</Button>
    <Button type="button" disabled={!enabled('number', 'text', 'date')} onclick={() => onTemplate('', ' BETWEEN ', ' AND ')}>BETWEEN</Button><Button type="button" disabled={!ready} onclick={() => onInsert(' IS NULL')}>IS NULL</Button><Button type="button" disabled={!ready} onclick={() => onInsert(' IS NOT NULL')}>IS NOT NULL</Button>
  </FormulaButtonGroup>
  <FormulaButtonGroup label="Logic" {keypad} available={keypad && (enabled('boolean') || editable)}>
    <Button type="button" disabled={!enabled('boolean')} onclick={() => onInsert(' AND ')}>AND</Button><Button type="button" disabled={!enabled('boolean')} onclick={() => onInsert(' OR ')}>OR</Button><Button type="button" active={activeFunction === 'not'} disabled={!enabled('boolean')} onclick={() => onWrap('NOT (', ')')}>NOT</Button>
    <Button type="button" disabled={!editable} onclick={() => onReplace('TRUE')}>TRUE</Button><Button type="button" disabled={!editable} onclick={() => onReplace('FALSE')}>FALSE</Button>
  </FormulaButtonGroup>
  <FormulaButtonGroup label="Number" {keypad} available={keypad && enabled('number')}>
    <Button type="button" active={activeFunction === 'abs'} disabled={!enabled('number')} onclick={() => onWrap('abs(', ')')}>abs</Button><Button type="button" active={activeFunction === 'round'} disabled={!enabled('number')} onclick={() => onWrap('round(', ')')}>round</Button><Button type="button" active={activeFunction === 'sqrt'} disabled={!enabled('number')} onclick={() => onWrap('sqrt(', ')')}>sqrt</Button><Button type="button" active={activeFunction === 'power'} disabled={!enabled('number')} onclick={() => onWrap('power(', ', 2)')}>power</Button>
    <Button type="button" active={activeFunction === 'cast'} disabled={!enabled('number')} onclick={() => onWrap('cast(', ' AS VARCHAR)')}>to text</Button>
  </FormulaButtonGroup>
  <FormulaButtonGroup label="Date" {keypad} available={keypad && enabled('date')}>
    <Button type="button" active={activeFunction === 'year'} disabled={!enabled('date')} onclick={() => onWrap('year(', ')')}>year</Button><Button type="button" active={activeFunction === 'month'} disabled={!enabled('date')} onclick={() => onWrap('month(', ')')}>month</Button><Button type="button" active={activeFunction === 'day'} disabled={!enabled('date')} onclick={() => onWrap('day(', ')')}>day</Button><Button type="button" active={activeFunction === 'date_diff'} disabled={!enabled('date')} onclick={() => onTemplate("date_diff('day', ", ', ', ')')}>date_diff</Button>
  </FormulaButtonGroup>
  <FormulaButtonGroup label="Text" {keypad} wide={keypad} available={keypad && enabled('text')}>
    <Button type="button" active={activeFunction === 'upper'} disabled={!enabled('text')} onclick={() => onWrap('upper(', ')')}>upper</Button><Button type="button" active={activeFunction === 'lower'} disabled={!enabled('text')} onclick={() => onWrap('lower(', ')')}>lower</Button><Button type="button" active={activeFunction === 'trim'} disabled={!enabled('text')} onclick={() => onWrap('trim(', ')')}>trim</Button><Button type="button" active={activeFunction === 'length'} disabled={!enabled('text')} onclick={() => onWrap('length(', ')')}>length</Button>
    <Button type="button" active={activeFunction === 'try_cast'} disabled={!enabled('text')} onclick={() => onWrap('try_cast(', ' AS DOUBLE)')}>to number</Button><Button type="button" active={activeFunction === 'left'} disabled={!enabled('text')} onclick={() => onWrap('left(', ', 1)')}>left</Button><Button type="button" active={activeFunction === 'right'} disabled={!enabled('text')} onclick={() => onWrap('right(', ', 1)')}>right</Button><Button type="button" active={activeFunction === 'regexp_extract'} disabled={!enabled('text')} onclick={() => onTemplate('regexp_extract(', ", '", "')")}>regexp_extract</Button>
    <Button type="button" disabled={!enabled('text')} onclick={() => onInsert(' || ')}>concat</Button><Button type="button" disabled={!enabled('text')} onclick={() => onInsert(` || ${quoteLiteral(' ')}`)}>space</Button><Button type="button" disabled={!enabled('text')} onclick={() => onInsert(` || ${quoteLiteral(', ')}`)}>comma</Button><Button type="button" disabled={!enabled('text')} onclick={() => onInsert(` || ${quoteLiteral('-')}`)}>hyphen</Button>
  </FormulaButtonGroup>
  {@render conditional?.()}
</div>

<style>
  .helper-groups { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; }
  .helper-groups.keypad { grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; }
  @media (max-width: 720px) { .helper-groups:not(.keypad) { grid-template-columns: 1fr; } }
  @media (max-width: 560px) { .helper-groups.keypad { grid-template-columns: repeat(2, minmax(0, 1fr)); } }
  @media (max-width: 420px) { .helper-groups.keypad { grid-template-columns: 1fr; } }
</style>
