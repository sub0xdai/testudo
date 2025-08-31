# Project Best Practices

This document outlines the best practices for structuring and developing projects within this repository. It is based on the existing structure and conventions, which emphasize modularity, clear documentation, and a robust development process.

## 1. Modular Architecture

The project is organized into a modular architecture, with each distinct service or component residing in its own top-level directory. This promotes separation of concerns, independent development, and scalability.

**Key Principles:**

- **One Service, One Directory:** Each service (e.g., `frontend`, `backend`, `api`) should have its own dedicated directory.
- **Independent Dependencies:** Each service should manage its own dependencies (e.g., `package.json`, `requirements.txt`, `go.mod`).
- **Containerization:** Every service should be containerizable, with a `Dockerfile` in its root directory. This ensures consistent deployment and isolation.

## 2. Directory Structure

A consistent directory structure should be maintained within each service. This makes it easier to navigate the codebase and locate files.

**Recommended Structure:**

```
/service-name
├───/src/ or /<service_name>/ or /internal/  # Main source code
├───/tests/                 # Test files
├───/docs/                  # Service-specific documentation
├───/assets/                # Static assets (images, etc.)
├───/scripts/               # Build or utility scripts
├───Dockerfile              # Containerization instructions
├───README.md               # Service-specific overview
├───CHANGELOG.md            # Service-specific changelog
└───design.md               # Design and architecture decisions
```

## 3. AI-Driven Development Workflow

This project utilizes an AI-driven development workflow, with AI-generated documentation and context stored in `CLAUDE.md` files.

**Key Principles:**

- **`CLAUDE.md` in Every Directory:** Every directory and subdirectory should contain a `CLAUDE.md` file.
- **Contextual Documentation:** Each `CLAUDE.md` file should contain the AI-generated documentation, context, and decision-making process relevant to that specific directory. This provides a granular and contextual history of the AI's involvement.
- **Consistency:** This practice should be followed consistently across all services and modules to ensure a unified and well-documented codebase.

## 4. Development Frameworks and Principles

The following development frameworks and principles should be adhered to:

### Test-Driven Development (TDD)

- **Write Tests First:** Before writing any new code, a corresponding test should be written.
- **Comprehensive Test Coverage:** Aim for high test coverage to ensure code quality and prevent regressions.
- **Dedicated Test Directories:** All tests should be located in a `tests` directory within the service's root.

### Don't Repeat Yourself (DRY)

- **Reusable Components:** Create reusable components and modules to avoid duplicating code.
- **Modular Design:** The modular architecture of the project is a key enabler of the DRY principle.
- **Configuration Management:** Use configuration files to manage environment-specific settings and avoid hardcoding values.

## 5. Version Control

- **Git:** All code should be managed using Git.
- **`.gitignore`:** Each service should have its own `.gitignore` file to exclude unnecessary files from version control.
- **Commit Messages:** Write clear and concise commit messages that explain the "why" behind the changes.

By following these best practices, we can ensure that the project remains well-organized, maintainable, and scalable as it continues to evolve.
