// https://docs.rs/parry3d/0.23.0/parry3d/shape/trait.Shape.html
//
// This means we must implement PointQuery
// This is kinda tricky (we need to identify the closest point by using some kind of spiraling neighbor search)
// https://docs.rs/parry3d/0.23.0/parry3d/query/point/trait.PointQuery.html
//
// as well as RayCast (Easy, just copy pre-existing dda code)
// https://docs.rs/parry3d/0.23.0/parry3d/query/trait.RayCast.html
