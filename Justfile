set dotenv-load := false
set default-list := true

mongo_container := "khan-mongo-test"
mongo_port := "27017"
default_mongodb_uri := "mongodb://127.0.0.1:" + mongo_port + "/?directConnection=true&serverSelectionTimeoutMS=1000"
mongodb_uri := env_var_or_default("KHAN_TEST_MONGODB_URI", default_mongodb_uri)
external_mongodb_uri := env_var_or_default("KHAN_TEST_MONGODB_URI", "")
mongo_up_message := if external_mongodb_uri == "" { ":" } else { "echo 'Using KHAN_TEST_MONGODB_URI; skipping local MongoDB container startup.'" }
mongo_remove_existing := if external_mongodb_uri == "" { "docker rm -f " + mongo_container + " >/dev/null 2>&1" } else { ":" }
mongo_run := if external_mongodb_uri == "" { "docker run --rm -d --name " + mongo_container + " -p 127.0.0.1:" + mongo_port + ":27017 mongo:5.0.6 --replSet rs --bind_ip_all >/dev/null" } else { ":" }
mongo_wait_ping := if external_mongodb_uri == "" { "until docker exec " + mongo_container + " mongosh --quiet --eval 'quit(db.runCommand({ ping: 1 }).ok ? 0 : 1)' >/dev/null 2>&1; do sleep 1; done" } else { ":" }
mongo_init_repl_set := if external_mongodb_uri == "" { "docker exec " + mongo_container + " mongosh --quiet --eval 'try { rs.status().ok } catch (e) { rs.initiate({ _id: \"rs\", members: [{ _id: 0, host: \"localhost:27017\" }] }).ok }' >/dev/null 2>&1" } else { ":" }
mongo_wait_primary := if external_mongodb_uri == "" { "until docker exec " + mongo_container + " mongosh --quiet --eval 'quit(db.hello().isWritablePrimary ? 0 : 1)' >/dev/null 2>&1; do sleep 1; done" } else { ":" }
mongo_down_message := if external_mongodb_uri == "" { ":" } else { "echo 'Using KHAN_TEST_MONGODB_URI; no local MongoDB container to remove.'" }

mongo-up:
    @{{mongo_up_message}}
    @-{{mongo_remove_existing}}
    @{{mongo_run}}
    @{{mongo_wait_ping}}
    @{{mongo_init_repl_set}}
    @{{mongo_wait_primary}}

mongo-down:
    @{{mongo_down_message}}
    @-{{mongo_remove_existing}}

clippy:
    cargo clippy --manifest-path khan/Cargo.toml --all-targets --all-features --no-deps

test: mongo-up && mongo-down
    KHAN_TEST_MONGODB_URI='{{mongodb_uri}}' cargo test --manifest-path khan/Cargo.toml --all-features

check: clippy test
