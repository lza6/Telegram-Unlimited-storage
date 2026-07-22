\set ON_ERROR_STOP on
\getenv saga_node_id SAGA_NODE_ID
\getenv saga_node_token SAGA_NODE_TOKEN
BEGIN;
CREATE TEMP TABLE scheduler_test_secret(node_id text,node_token text) ON COMMIT DROP;
INSERT INTO scheduler_test_secret VALUES (:'saga_node_id', :'saga_node_token');
DO $test$
DECLARE n text; t text; tenant uuid := '10000000-0000-4000-8000-000000000001'; job uuid := '20000000-0000-4000-8000-000000000001'; l record;
BEGIN
 SELECT node_id,node_token INTO n,t FROM scheduler_test_secret;
 INSERT INTO tenants(id,slug,display_name) VALUES(tenant,'scheduler-test','Scheduler Test') ON CONFLICT(id) DO NOTHING;
 INSERT INTO transfer_jobs(id,tenant_id,direction,idempotency_key,status,correlation_id)
 VALUES(job,tenant,'upload','scheduler-test','running','30000000-0000-4000-8000-000000000001')
 ON CONFLICT(id) DO UPDATE SET status='running';

 BEGIN
   PERFORM authenticate_saga_node(n,t||'-wrong',false,false);
   RAISE EXCEPTION 'wrong token unexpectedly authenticated';
 EXCEPTION WHEN insufficient_privilege THEN NULL;
 END;

 SELECT * INTO l FROM acquire_transfer_scheduler_lease(n,t,job,ARRAY[
   'global:bot:upload','tenant:'||tenant::text||':upload','bot:test-bot:upload','peer:bot:channel:-1001:upload'
 ],30);
 IF l.lease_id IS NULL OR l.fence_token < 1 THEN RAISE EXCEPTION 'scheduler lease missing'; END IF;

 BEGIN
   PERFORM renew_transfer_scheduler_lease(n,t,l.lease_id,l.attempt_token,l.fence_token+1,30);
   RAISE EXCEPTION 'stale fence unexpectedly renewed';
 EXCEPTION WHEN SQLSTATE '55000' THEN NULL;
 END;

 PERFORM renew_transfer_scheduler_lease(n,t,l.lease_id,l.attempt_token,l.fence_token,30);
 PERFORM set_transfer_scheduler_cooldown(n,t,'peer:bot:channel:-1001:upload',1,'TEST_FLOOD_WAIT');
 PERFORM finish_transfer_scheduler_lease(n,t,l.lease_id,l.attempt_token,l.fence_token,'success',NULL,NULL,NULL);

 IF EXISTS(SELECT 1 FROM scheduler_resources WHERE active_leases<>0 AND resource_key=ANY(ARRAY[
   'global:bot:upload','tenant:'||tenant::text||':upload','bot:test-bot:upload','peer:bot:channel:-1001:upload'
 ])) THEN RAISE EXCEPTION 'scheduler resources leaked active leases'; END IF;
 RAISE NOTICE 'POSTGRES_SCHEDULER_ROUNDTRIP=1';
END
$test$;
ROLLBACK;