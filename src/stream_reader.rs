//stream the book on a per byte basis
//this can be done with http response headers
//use tower-http for when the files are large to serve partial files
//the webview in flutter should be able to support partial views

use log::warn;
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncSeekExt},
};

use axum::{
    body::Body,
    http::{
        HeaderMap, HeaderValue, Request, Response, StatusCode, header::{self, RANGE}
    },
    response::IntoResponse,
};

use crate::{
    error_types::StreamReaderErrors,
    models::{BookFormat, BookMetadata},
};
use tokio_util::io::ReaderStream;
use tower_http;

pub async fn streaming_file(
    metadata: &BookMetadata,
    request: Request<()>,
) -> Result<impl IntoResponse, StreamReaderErrors> {
    let mut file = File::open(&metadata.file_path)
        .await
        .map_err(|_| StreamReaderErrors::FileNotFound)?;
    let file_size = file
        .metadata()
        .await
        .map_err(|_| StreamReaderErrors::FileNotFound)?
        .len();
    let parsed_header = get_parsed_range_header(request.headers(), file_size)?;

    let content_type = get_content_type_header_value(metadata.clone().format)?;

    if let Some((start, end)) = parsed_header {
        let body = get_stream_body(file, (start, end)).await?;
        return get_single_partial_stream_response(&(start, end), file_size, body, content_type);
    }
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);
    get_full_stream_response(body, file_size, content_type)
}

async fn get_stream_body(mut file: File, range: (u64, u64)) -> Result<Body, StreamReaderErrors> {
    file.seek(std::io::SeekFrom::Start(range.0))
        .await
        .map_err(|_| StreamReaderErrors::BodyCreationFailed)?;
    let count = range.1 - range.0 + 1;
    let partial_read = file.take(count);
    let stream = ReaderStream::new(partial_read);
    let body = Body::from_stream(stream);
    Ok(body)
}

fn get_parsed_range_header(
    header: &HeaderMap,
    file_size: u64,
) -> Result<Option<(u64, u64)>, StreamReaderErrors> {
    let parts = match get_header_parts(header){
        Ok(result) => result,
        Err(e) => {
            warn!("could not retrieve range header {:?}",e);
            return Err(StreamReaderErrors::InvalidRangeHeader);
        }
    };

    let start = parts.0;
    let end = parts.1;

    let (start, end) = match (start.is_empty(), end.is_empty()) {
        (false, false) => {
            let s = start
                .parse::<u64>()
                .map_err(|_| StreamReaderErrors::InvalidRangeHeader)?;
            let e = end
                .parse::<u64>()
                .map_err(|_| StreamReaderErrors::InvalidRangeHeader)?;
            (s, e)
        }
        (false, true) => {
            let s = start
                .parse::<u64>()
                .map_err(|_| StreamReaderErrors::InvalidRangeHeader)?;
            (s, file_size - 1)
        }
        (true, false) => {
            let last = end
                .parse::<u64>()
                .map_err(|_| StreamReaderErrors::InvalidRangeHeader)?;
            (file_size - last, file_size - 1)
        }
        _ => return Err(StreamReaderErrors::InvalidRangeHeader),
    };

    if start >= file_size || end >= file_size || start > end {
        return Err(StreamReaderErrors::InvalidRangeHeader);
    }

    Ok(Some((start, end)))
}

fn get_header_parts(header: &HeaderMap)->Result<(&str,&str),StreamReaderErrors>{
    if !header.contains_key(RANGE) {
        return Err(StreamReaderErrors::InvalidRangeHeader);
    }

    let range_part = header
        .get(RANGE)
        .ok_or(StreamReaderErrors::InvalidRangeHeader)?;

    let range_full_string = range_part
        .to_str()
        .map_err(|_| StreamReaderErrors::InvalidRangeHeader)?;

    if !range_full_string.starts_with("bytes=") {
        return Err(StreamReaderErrors::InvalidRangeHeader);
    }
    let range_string: Vec<&str> = range_full_string.split('=').collect();
    if range_string.len() != 2 {
        return Err(StreamReaderErrors::InvalidRangeHeader);
    }
    let parts: Vec<&str> = range_string[1].split('-').collect();
    if parts.len() != 2 {
        return Err(StreamReaderErrors::InvalidRangeHeader);
    }
    let start = parts[0];
    let end = parts[1];
    Ok((start,end))
}

fn get_content_type_header_value(format: BookFormat) -> Result<String, StreamReaderErrors> {
    //when the file to stream is pdf application/pdf
    //when epub -> something else
    //etc
    match format {
        BookFormat::Pdf => Ok("application/pdf".to_string()),
        BookFormat::Epub => Ok("application/epub+zip".to_string()),
        BookFormat::Cbz => Ok("application/zip".to_string()),
        BookFormat::ImageComic => Ok("image/jpg".to_string()), //for now we will assume all images are jpg images. Later on will add HEIC(maybe) and png as well
        BookFormat::None => Err(StreamReaderErrors::NoFileTypeFound),
    }
}

fn get_single_partial_stream_response(
    range_values: &(u64, u64),
    file_size: u64,
    body: Body,
    content_type: String,
) -> Result<Response<Body>, StreamReaderErrors> {
    //When the request needs to return a partial response
    //stream the requested bytes
    let range = range_values.1 - range_values.0 + 1;
    let mut response = Response::new(body);
    *response.status_mut() = StatusCode::PARTIAL_CONTENT;

    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::try_from(content_type)
            .map_err(|_| StreamReaderErrors::ResponseCreationFailed)?,
    );
    response.headers_mut().insert(
        header::CONTENT_LENGTH,
        HeaderValue::try_from(&range.to_string())
            .map_err(|_| StreamReaderErrors::ResponseCreationFailed)?,
    );
    response.headers_mut().insert(
        header::CONTENT_RANGE,
        HeaderValue::try_from(format!(
            "bytes {}-{}/{}",
            range_values.0, range_values.1, file_size
        ))
        .map_err(|_| StreamReaderErrors::ResponseCreationFailed)?,
    ); //bytes + range that will be sent/max range
    response
        .headers_mut()
        .insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));

    //for a multipart partial data stream this will need to be different as well
    Ok(response)
}

fn get_full_stream_response(
    body: Body,
    file_size: u64,
    content_type: String,
) -> Result<Response<Body>, StreamReaderErrors> {
    //when the requested bytes are the full document or there was no range given
    //return the full document

    let mut response = Response::new(body);
    *response.status_mut() = StatusCode::OK;

    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::try_from(content_type)
            .map_err(|_| StreamReaderErrors::ResponseCreationFailed)?,
    );
    response.headers_mut().insert(
        header::CONTENT_LENGTH,
        HeaderValue::try_from(&file_size.to_string())
            .map_err(|_| StreamReaderErrors::ResponseCreationFailed)?,
    );
    response
        .headers_mut()
        .insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));

    Ok(response)
}
