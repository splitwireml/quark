//! TIMETZ cells keep their stored offset in JSON and Arrow pages.

use super::*;
use arrow::array::AsArray;
use arrow::datatypes::DataType;

fn timetz_engine() -> Engine {
    engine(
        "CREATE TABLE t AS SELECT range::INTEGER AS id, \
         TIMETZ '12:30:00-02' + to_seconds(range) AS at FROM range(2)",
    )
}

#[test]
fn timetz_keeps_its_offset_in_json_pages() {
    let engine = timetz_engine();

    let body = json_body(
        fetch(
            &engine,
            &dataset("t"),
            &QueryRequest::default(),
            Format::Json,
        )
        .unwrap(),
    );

    assert_eq!(body["columns"][1]["type"], "TIME WITH TIME ZONE");
    assert_eq!(body["rows"][0]["at"], "12:30:00-02:00");
    assert_eq!(body["rows"][1]["at"], "12:30:01-02:00");
}

#[test]
fn timetz_keeps_its_offset_in_arrow_pages() {
    let engine = timetz_engine();

    let bytes = arrow_body(
        fetch(
            &engine,
            &dataset("t"),
            &QueryRequest::default(),
            Format::Arrow,
        )
        .unwrap(),
    );

    let reader = StreamReader::try_new(Cursor::new(bytes), None).unwrap();
    let meta: Value = serde_json::from_str(&reader.schema().metadata()["quark"]).unwrap();
    assert_eq!(meta["json_columns"], json!(["at"]));
    let batch = reader.map(Result::unwrap).next().unwrap();
    let cells = batch.column(1).as_string::<i32>();
    assert_eq!(cells.value(0), "\"12:30:00-02:00\"");
    assert_eq!(batch.column(0).data_type(), &DataType::Int32);
}
