import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { ESLint } from 'eslint'
import ts from 'typescript'

const filePath = 'src/__lint_policy_fixture__.tsx'
const lint = new ESLint()
const fixingLint = new ESLint({ fix: true })
const namingRule = '@typescript-eslint/naming-convention'

/** Compare import meaning as well as layout: type modifiers must survive auto-fixing. */
function namedImports(source) {
  const tree = ts.createSourceFile(filePath, source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX)
  const declaration = tree.statements.find(ts.isImportDeclaration)
  const bindings = declaration.importClause.namedBindings
  return bindings.elements.map(item => ({
    name: item.name.text,
    original: item.propertyName?.text,
    typeOnly: item.isTypeOnly || declaration.importClause.isTypeOnly,
    line: tree.getLineAndCharacterOfPosition(item.getStart(tree)).line,
  }))
}

test('named value/type imports auto-fix to one item per line without changing their meaning', async () => {
  for (const source of [
    "import { useRef, useCallback, type Dispatch } from 'react'\nexport { useRef, useCallback }\nexport type Handler = Dispatch<string>\n",
    "import type { RefObject, Dispatch } from 'react'\nexport type Pair = [RefObject<string>, Dispatch<string>]\n",
  ]) {
    const [before] = await lint.lintText(source, { filePath })
    assert(before.messages.some(message => message.ruleId === 'import-newlines/enforce'))
    const [fixed] = await fixingLint.lintText(source, { filePath })
    assert.equal(fixed.errorCount, 0)
    const original = namedImports(source)
    const formatted = namedImports(fixed.output)
    assert.deepEqual(formatted.map(({ line, ...item }) => item), original.map(({ line, ...item }) => item))
    assert.equal(new Set(formatted.map(item => item.line)).size, formatted.length)
    assert(!fixed.messages.some(message => message.ruleId === 'import-newlines/enforce'))
  }
})

test('object layout fixes while multiple statements still produce an explicit diagnostic', async () => {
  const [object] = await fixingLint.lintText('export const value = { first: 1, second: 2 }\n', { filePath })
  assert.match(object.output, /\{\n\s+first: 1,\n\s+second: 2,\n\}/)
  assert.equal(object.errorCount, 0)
  const [statements] = await fixingLint.lintText('export function update(value: number) { value++; value += 1; return value }\n', { filePath })
  assert(statements.messages.some(message => message.ruleId === '@stylistic/max-statements-per-line' && message.severity === 1))
})

test('only explicitly private class methods require underscore; public and #private methods do not', async () => {
  for (const [declaration, call, errors] of [
    ['private _compute()', 'this._compute()', 0],
    ['private compute()', 'this.compute()', 1],
    ['private static _compute()', 'Example._compute()', 0],
    ['#compute()', 'this.#compute()', 0],
    ['#_compute()', 'this.#_compute()', 1],
    ['_compute()', 'this._compute()', 1],
  ]) {
    const [result] = await lint.lintText(`export class Example { ${declaration} { return 1 } run() { return ${call} } }`, { filePath })
    assert.equal(result.messages.filter(message => message.ruleId === namingRule).length, errors, declaration)
  }
  const [helpers] = await lint.lintText('function localHelper() {}\nfunction useLocal() {}\nfunction Widget() {}\nexport const helpers = [localHelper, useLocal, Widget]\n', { filePath })
  assert(!helpers.messages.some(message => message.ruleId === namingRule))
})

test('correctness errors remain errors and the editor uses the same ESLint save action', async () => {
  const [result] = await lint.lintText('export const broken: any = 1\n', { filePath })
  assert(result.messages.some(message => message.ruleId === '@typescript-eslint/no-explicit-any' && message.severity === 2))
  const settings = JSON.parse(readFileSync('.vscode/settings.json', 'utf8'))
  assert.equal(settings['files.autoSave'], 'onFocusChange')
  assert.equal(settings['[javascript][javascriptreact][typescript][typescriptreact]']['editor.codeActionsOnSave']['source.fixAll.eslint'], 'always')
})
