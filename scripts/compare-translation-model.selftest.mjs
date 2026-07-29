import assert from 'node:assert/strict'
import test from 'node:test'

import {
  buildTranslationRequest,
  tokenBigramDice,
} from './compare-translation-model.mjs'

test('tokenBigramDice treats identical text as a full match', () => {
  assert.equal(tokenBigramDice('Once again, the class acts.', 'Once again the class acts'), 1)
})

test('tokenBigramDice rewards sequence rather than a bag of words', () => {
  assert.ok(tokenBigramDice('the class acts independently', 'the class acts independently') >
    tokenBigramDice('independently acts class the', 'the class acts independently'))
})

test('buildTranslationRequest uses the official TranslateGemma language fields', () => {
  const request = buildTranslationRequest({
    sourceLanguage: 'es',
    targetLanguage: 'en',
  }, {
    source: 'Un año más.',
  }, 'local-model')

  assert.deepEqual(request.messages[0].content[0], {
    type: 'text',
    source_lang_code: 'es',
    target_lang_code: 'en',
    text: 'Un año más.',
  })
})
