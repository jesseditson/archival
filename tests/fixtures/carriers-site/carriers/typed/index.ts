type Secrets = { settings: { api_key: string | null } };

// A carrier reads secret fields; templates never can.
export default (_params: URLSearchParams, _body: unknown, objects: Secrets) =>
  new Response(objects.settings.api_key ?? "", {
    headers: { "content-type": "text/plain" },
  });
