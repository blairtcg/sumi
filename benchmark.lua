-- https://github.com/wg/wrk
--
-- this is our benchmarking script used for testing how long
-- it takes for sumi to render drop image requests under heavy load.
-- if sumi takes 100 requests all in one moment, sumi can handle
-- 30-70 image requests per second depending on cpu cores and specs.
-- with decent cpu specs and 4 cores it takes an average of ~120ms
-- for every image rendered and can deal with ~35 rps.
-- sumi will likely deal with a maximum of 2s latency on heavy demand
-- and will "never" exceed our default 10s timeout so it will never fail fast.

local cards = {}
for line in io.lines("cards.txt") do
	if line ~= "" then
		table.insert(cards, line)
	end
end

local thread_counter = 1
function setup(thread)
	thread:set("id", thread_counter)
	thread_counter = thread_counter + 1
end

function init()
	math.randomseed(os.time() + id)
end

function request()
	local path = string.format(
		"/render/drop?left=%s&right=%s&left_print=%d&right_print=%d",
		cards[math.random(#cards)],
		cards[math.random(#cards)],
		math.random(999),
		math.random(999)
	)
	return wrk.format("GET", path)
end
