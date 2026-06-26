set dotenv-load := false

mongo_container := "khan-mongo-test"
mongo_port := "27017"
default_mongodb_uri := "mongodb://127.0.0.1:" + mongo_port + "/?directConnection=true&serverSelectionTimeoutMS=1000"
mongodb_uri := env_var_or_default("KHAN_TEST_MONGODB_URI", default_mongodb_uri)

default:
    @just --list

mongo-up:
    @if [ -n "$KHAN_TEST_MONGODB_URI" ]; then \
        echo 'Using KHAN_TEST_MONGODB_URI; skipping local MongoDB container startup.'; \
        exit 0; \
    fi
    @if docker inspect {{mongo_container}} >/dev/null 2>&1; then \
        docker start {{mongo_container}} >/dev/null; \
    else \
        docker run -d --name {{mongo_container}} -p 127.0.0.1:{{mongo_port}}:27017 mongo:5.0.6 --replSet rs --bind_ip_all >/dev/null; \
    fi
    @until docker exec {{mongo_container}} mongosh --quiet --eval 'db.runCommand({ ping: 1 }).ok' | grep -q 1; do sleep 1; done
    @docker exec {{mongo_container}} mongosh --quiet --eval 'try { rs.status().ok } catch (e) { rs.initiate({ _id: "rs", members: [{ _id: 0, host: "localhost:27017" }] }).ok }' >/dev/null
    @until docker exec {{mongo_container}} mongosh --quiet --eval 'db.hello().isWritablePrimary' | grep -q true; do sleep 1; done

mongo-down:
    @if [ -n "$KHAN_TEST_MONGODB_URI" ]; then \
        echo 'Using KHAN_TEST_MONGODB_URI; no local MongoDB container to remove.'; \
        exit 0; \
    fi
    @docker rm -f {{mongo_container}} >/dev/null 2>&1 || true

mongo-logs:
    @if [ -n "$KHAN_TEST_MONGODB_URI" ]; then \
        echo 'Using KHAN_TEST_MONGODB_URI; no local MongoDB container logs are available.'; \
        exit 0; \
    fi
    @docker logs {{mongo_container}}

test-doc: mongo-up
    KHAN_TEST_MONGODB_URI='{{mongodb_uri}}' cargo test --manifest-path khan/Cargo.toml --all-features --doc

test: mongo-up
    KHAN_TEST_MONGODB_URI='{{mongodb_uri}}' cargo test --manifest-path khan/Cargo.toml --all-features

clippy:
    cargo clippy --manifest-path khan/Cargo.toml --all-targets --all-features --no-deps

check: mongo-up
    KHAN_TEST_MONGODB_URI='{{mongodb_uri}}' cargo test --manifest-path khan/Cargo.toml --all-features --doc
    KHAN_TEST_MONGODB_URI='{{mongodb_uri}}' cargo test --manifest-path khan/Cargo.toml --all-features
    cargo clippy --manifest-path khan/Cargo.toml --all-targets --all-features --no-deps
