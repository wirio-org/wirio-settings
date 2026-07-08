# AGENTS.md

# Environment

- Lint: `make lint`.
- Test: `make test`.

# General guidelines

- Don't modify the `_wirio_settings.pyi` file manually. It's generated automatically by `maturin`. If you need to modify it, do so in the `lib.rs` file and then run `make generate-stubs` to regenerate the `.pyi` file.
- Review the added code and write comprehensive unit tests to achieve 100% test coverage. Use `cargo llvm-cov --text` to check the coverage.
- Don't stop until the next steps, in order, pass: linter and tests.
- After all steps, review if the documentation needs to be updated. If it does, update it accordingly.

# Rust

## Code style

#### General guidelines

- When `expect` is used, provide a message that describes the reason for the expectation. It must start with a capital letter and not end with a period.
- Don't modify the `_wirio_settings.pyi` file manually. It is generated automatically by `maturin`. If you need to modify it, do so in the `lib.rs` file and then run `make generate-stubs` to regenerate the `.pyi` file.
- When using PyO3, use attributes instead of functions such as `.add_function` or `add_submodule`.

## Testing

- Test names must start with `test_`, be followed by a verb in present tense, and read as `The test should...`. The names mustn't include the word "should", and they must be descriptive and concise, avoiding the inclusion of the tested function whenever possible. Examples: `test_create_user`, `test_fail_when_creating_user_with_untrusted_email`, `test_ban_user_using_administrator_account`.
- Append new test cases to the end of the existing ones.
- Use `unwrap` instead of `expect` in tests.
- Instead of creating structs for testing, use the `mockall` crate for mocking.
- Mock variables must end with `_mock`. For example, `configuration_mock`.
- To create temporary files in tests, use the `tempfile` crate.
- Asynchronous tests must be annotated using `#[tokio::test]` instead of `#[test]`.

# Python

## Environment

- Package manager: `uv`.

## Code style

### General guidelines

- Use type hints everywhere.
- Use standard decorators, such as `@override` or `@abstractmethod`, when appropriate.
- Use `@final` for classes that are not meant to be inherited from, and for methods that are not meant to be overridden (if the class is not already marked as final).
- Don't create standalone functions outside the class for class-specific logic.
- Don't create aliases.
- When adding private methods (those starting with an underscore), add them at the end of the class, after all public methods.

### Explicit comparisons in conditionals

- Do **not** rely on Python truthiness in conditionals. Agents mustn't generate `if value:` or `if not value:`, except when checking booleans.
- Use explicit comparisons that match the intended meaning.

**Examples of allowed uses:**

```python
if value is None
if value is not None

if len(items) == 0
if len(items) > 0

if count == 0
if count != 0
if count > 0

# Boolean values
if flag
if not flag
```

**Examples of forbidden uses:**

```python
if value
if not value
while value
return value or default
```

**Replacement guide:**

| Case           | Use                 |
| -------------- | ------------------- |
| optional value | `value is not None` |
| collection     | `len(value) > 0`    |
| number         | `value != 0`        |
| boolean        | `value`             |

## Testing

- Run tests using `uv`.
- Test files must be placed in the `tests` directory, mirroring the structure of the `src` directory.
- Test names must start with `test_`, be followed by a verb in present tense, and read as `The test should...`. The names mustn't include the word "should", and they must be descriptive and concise, avoiding the inclusion of the tested function whenever possible. Examples: `test_create_user`, `test_fail_when_creating_user_with_untrusted_email`, `test_ban_user_using_administrator_account`.
- Append new test cases to the end of the existing ones.
- When mocks are needed, use `MockerFixture`.
- `MockerFixture` should use the `create_autospec` method to create mocks, and the `instance=True` parameter should be passed to ensure the mock behaves like an instance of the class being mocked. `mocker.Mock()` should be avoided, as it creates a generic mock that doesn't enforce the interface of the mocked class. Use `MockerFixture` instead of `monkeypatch`.
- When patching, don't hardcode the path. If you have to reference a module, use `__module__`, and if you have to reference a method, use `__name__`. For example, instead of `mocker.patch("HostEnvironment.inspect.currentframe")`, use `mocker.patch(f"{HostEnvironment.__module__}.{inspect.__name__}.{inspect.currentframe.__name__}")`. Also, when patching, use `autospec=True` to ensure the mock behaves like the original object.

# Documentation

- Use American English.
- The documentation must be placed in the `docs` directory or the `README` file.
- Use "we" to refer to the reader and the author together.
