ARG PYTHON_BASE=3.9-slim
FROM python:$PYTHON_BASE AS builder

# install PDM
RUN pip install -U pdm
ENV PDM_CHECK_UPDATE=false
COPY pyproject.toml pdm.lock README.md /project/
COPY tg_searcher /project/tg_searcher

# install dependencies and project into the local packages directory
WORKDIR /project
RUN pdm install --check --prod --no-editable

# run stage
FROM python:$PYTHON_BASE

# retrieve packages from build stage
COPY --from=builder /project/.venv/ /project/.venv
ENV PATH="/project/.venv/bin:$PATH"
COPY tg_searcher /project/tg_searcher
WORKDIR /project
CMD ["python", "tg_searcher/__main__.py"]

