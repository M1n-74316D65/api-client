use crate::types::HttpMethod;

pub async fn execute_request(
    url: &str,
    method: &HttpMethod,
    body: &str,
    headers: &[(String, String)],
) -> Result<(u16, String), String> {
    let client = reqwest::Client::new();

    let mut builder = match method {
        HttpMethod::Get => client.get(url),
        HttpMethod::Post => client.post(url),
        HttpMethod::Put => client.put(url),
        HttpMethod::Delete => client.delete(url),
        HttpMethod::Patch => client.patch(url),
    };

    // Add headers
    for (key, value) in headers {
        builder = builder.header(key.as_str(), value.as_str());
    }

    // Add body for methods that support it
    if !body.is_empty()
        && matches!(
            method,
            HttpMethod::Post | HttpMethod::Put | HttpMethod::Patch
        )
    {
        builder = builder.body(body.to_string());
    }

    let response = builder.send().await.map_err(|e| e.to_string())?;
    let status = response.status().as_u16();
    let text = response.text().await.map_err(|e| e.to_string())?;

    Ok((status, text))
}
