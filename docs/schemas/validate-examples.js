#!/usr/bin/env node
/**
 * HEIMDALL schema validation — CI script
 *
 * Validates every example file in docs/schemas/examples/ against its
 * corresponding schema in docs/schemas/. Run with:
 *
 *   node docs/schemas/validate-examples.js
 *
 * Requires: npm install -g ajv-cli  (or: npx ajv-cli)
 * Or use the npm script: npm run validate-schemas
 *
 * Exit code 0 = all examples valid. Non-zero = at least one failure.
 */

"use strict";

const fs = require("fs");
const path = require("path");
const Ajv = require("ajv");
const addFormats = require("ajv-formats");

const SCHEMAS_DIR = path.join(__dirname);
const EXAMPLES_DIR = path.join(__dirname, "examples");

// Map from example filename prefix to schema filename
const EXAMPLE_TO_SCHEMA = {
  "enrol-request": "enrol-request.schema.json",
  "enrol-response-enrolled": "enrol-response.schema.json",
  "enrol-response-refused": "enrol-response.schema.json",
  "heartbeat": "heartbeat.schema.json",
  "measurement": "measurement.schema.json",
  "lease-request": "lease-request.schema.json",
  "lease-response-granted": "lease-response.schema.json",
  "lease-response-refused": "lease-response.schema.json",
  "failure-report": "failure-report.schema.json",
  "spool-record-measurement": "spool-record.schema.json",
  "spool-record-shutdown": "spool-record.schema.json",
  "spool-record-boot": "spool-record.schema.json",
};

function loadJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

function main() {
  const ajv = new Ajv({ strict: false, allErrors: true });
  addFormats(ajv);

  let passed = 0;
  let failed = 0;

  const examples = fs
    .readdirSync(EXAMPLES_DIR)
    .filter((f) => f.endsWith(".example.json"))
    .sort();

  for (const exampleFile of examples) {
    const prefix = exampleFile.replace(".example.json", "");
    const schemaName = EXAMPLE_TO_SCHEMA[prefix];

    if (!schemaName) {
      console.error(`SKIP  ${exampleFile} — no schema mapping defined`);
      continue;
    }

    const schemaPath = path.join(SCHEMAS_DIR, schemaName);
    const examplePath = path.join(EXAMPLES_DIR, exampleFile);

    const schema = loadJson(schemaPath);
    const example = loadJson(examplePath);

    // Remove $schema keyword that older drafts may not support
    const ajvSchema = Object.assign({}, schema);
    delete ajvSchema["$schema"];

    const validate = ajv.compile(ajvSchema);
    const valid = validate(example);

    if (valid) {
      console.log(`PASS  ${exampleFile}`);
      passed++;
    } else {
      console.error(`FAIL  ${exampleFile}`);
      for (const err of validate.errors || []) {
        console.error(`      ${err.instancePath || "/"} ${err.message}`);
      }
      failed++;
    }
  }

  console.log(`\n${passed} passed, ${failed} failed`);
  process.exit(failed > 0 ? 1 : 0);
}

main();
