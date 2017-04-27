permissions! {
// These are the roles. Lower numbers have less privileges.
// Higher ranks automatically have lower ranks' privileges.
// Make sure you have the lowest one at zero, because that's the default.
[ member = 0 | admin = 100 ]

// These are the permissions.
// The first bit is the privilege name. Don't change this.
// Everything afterwards is the condition that needs to be met.
// There are the following conditions:
// - has_role(ROLE): The user's rank needs to meet or exceed the given rank.
// - own: The thing the user's trying to affect belongs to them.
// - own_student: The thing the user's trying to affect belongs to one of their students.
// - any(CONDITION, ...): One or more of the given conditions needs to be met.
// - all(CONDITION, ...): All of the given conditions need to be met.
// - anyone: Use this if there are no conditions to be met.
// Note: `own` and `own_student` only make sense in the context of modifying user data. 

[ create_user has_role(admin) ]
[ delete_user has_role(admin) ]

[ read_name anyone ]
[ read_username anyone ]
[ search_users anyone ]
[ read_email_address any(own, has_role(admin), own_student) ]

[ edit_name any(own, has_role(admin)) ]
[ edit_username any(own, has_role(admin)) ]
[ edit_email_address any(own, has_role(admin)) ]

[ edit_password any(own, has_role(admin)) ]
[ read_role any(own, has_role(admin)) ]
[ edit_role has_role(admin) ]

[ create_location has_role(admin) ]
[ read_location_info anyone ]
[ edit_location_info has_role(admin) ]
[ delete_location has_role(admin) ]

[ read_students any(own, own_student, has_role(admin)) ]
[ edit_students any(own_student, has_role(admin)) ]
[ read_instructors anyone ]
[ edit_instructors has_role(admin) ]
}
